use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;

use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceEventLifecycle,
    RegistryOwnerLifecycle, RegistryPublishRequestLifecycle, RegistryReleaseLifecycle,
    RegistryValidationStageLifecycle,
};
use crate::entities::module::{MarketplaceModule, ModuleSettingField, TenantModule};
use crate::features::modules::api::{self, RegistryMutationResult};
use crate::shared::ui::{Button, Input};
use crate::{use_i18n, Locale};

#[derive(Clone)]
struct MetadataChecklistItem {
    label: &'static str,
    state: &'static str,
    priority: &'static str,
    summary: &'static str,
    detail: String,
}

#[derive(Clone)]
struct RegistryLiveApiActionHint {
    endpoint: String,
    authority: String,
    note: Option<String>,
    body_hint: Option<String>,
    header_hint: Option<String>,
    xtask_hint: Option<String>,
    write_path: bool,
}

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

fn latest_active_registry_version(module: &MarketplaceModule) -> Option<&MarketplaceModuleVersion> {
    module.versions.iter().find(|version| !version.yanked)
}

fn registry_governance_hint(module: &MarketplaceModule, locale: Locale) -> String {
    match (
        module.ownership.as_str(),
        module
            .registry_lifecycle
            .as_ref()
            .and_then(|lifecycle| lifecycle.latest_request.as_ref()),
        module
            .registry_lifecycle
            .as_ref()
            .and_then(|lifecycle| lifecycle.latest_release.as_ref()),
    ) {
        ("first_party", Some(request), _) if status_eq(&request.status, "rejected") => tr(
            locale,
            "Request needs operator follow-up before this module can be published again.",
            "Запросу требуется доработка оператором, прежде чем модуль можно будет снова публиковать.",
        )
        .to_string(),
        ("first_party", Some(_), Some(release)) if status_eq(&release.status, "yanked") => tr(
            locale,
            "Latest published release is yanked; future publish/yank actions should preserve the audit trail.",
            "Последний опубликованный релиз отозван; дальнейшие publish/yank-действия должны сохранять аудит-след.",
        )
        .to_string(),
        ("first_party", Some(_), _) => tr(
            locale,
            "First-party module is already tracked by the V2 publish lifecycle.",
            "First-party модуль уже находится под управлением V2 publish lifecycle.",
        )
        .to_string(),
        ("first_party", None, _) => tr(
            locale,
            "First-party modules can create V2 publish requests from a full host or through cargo xtask.",
            "First-party модули могут создавать V2 publish-запросы с full host или через cargo xtask.",
        )
        .to_string(),
        _ => tr(
            locale,
            "Third-party ownership still needs richer governance/moderation flow before live publish should be treated as production-ready.",
            "Для third-party ownership всё ещё нужен более полный governance/moderation flow, прежде чем live publish можно будет считать production-ready.",
        )
        .to_string(),
    }
}

fn registry_request_status_badge_classes(status: &str) -> &'static str {
    if status_eq(status, "published") || status_eq(status, "active") {
        "inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground"
    } else if status_eq(status, "rejected") || status_eq(status, "yanked") {
        "inline-flex items-center rounded-full border border-red-300 bg-red-50 px-2.5 py-0.5 text-xs font-semibold text-red-700"
    } else {
        "inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground"
    }
}

fn status_eq(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}

fn registry_mutation_result_summary(result: &RegistryMutationResult, locale: Locale) -> String {
    match result.status.as_deref() {
        Some(status) => format!(
            "{}: {}",
            tr(locale, "Action result", "Результат действия"),
            humanize_token(status)
        ),
        None => format!(
            "{}: {}",
            tr(locale, "Action result", "Результат действия"),
            humanize_token(&result.action)
        ),
    }
}

fn destructive_governance_action_label(action: &str, locale: Locale) -> &'static str {
    match action {
        "reject" => tr(locale, "Reject", "Отклонить"),
        "owner-transfer" => tr(locale, "Owner transfer", "Передать владение"),
        "yank" => tr(locale, "Yank", "Отозвать"),
        _ => tr(locale, "Confirm action", "Подтвердить действие"),
    }
}

fn destructive_governance_confirmation_message(
    action: &str,
    module_slug: &str,
    release_version: Option<&str>,
    new_owner_actor: Option<&str>,
    locale: Locale,
) -> String {
    match action {
        "reject" => format!(
            "{} `{}`. {}",
            tr(
                locale,
                "Reject the current publish request for module",
                "Отклонить текущий publish-запрос для модуля"
            ),
            module_slug,
            tr(
                locale,
                "This is a live governance decision and it will be written to the audit trail.",
                "Это live governance-решение, оно будет записано в аудит-след."
            )
        ),
        "owner-transfer" => format!(
            "{} `{}` {} `{}`. {}",
            tr(
                locale,
                "Transfer ownership for module",
                "Передать владение для модуля"
            ),
            module_slug,
            tr(locale, "to", "к"),
            new_owner_actor.unwrap_or("<new-owner-actor>"),
            tr(
                locale,
                "This rebinding is live and affects future publish and review authority.",
                "Эта привязка выполняется в live-режиме и влияет на будущие publish- и review-права."
            )
        ),
        "yank" => format!(
            "{} `{}`{} . {}",
            tr(locale, "Yank release for module", "Отозвать релиз модуля"),
            module_slug,
            release_version
                .map(|version| format!(" v{version}"))
                .unwrap_or_default(),
            tr(
                locale,
                "The release will leave the active catalog trail and remain only in history.",
                "Релиз уйдёт из активного каталога и останется только в истории."
            )
        ),
        _ => tr(
            locale,
            "Confirm the live governance action.",
            "Подтвердите live governance-действие."
        )
        .to_string(),
    }
}

fn copy_text_to_clipboard(_value: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(_value);
        }
    }
}

fn curl_snippet_for_live_api_action(item: &RegistryLiveApiActionHint) -> Option<String> {
    let (method, path) = item.endpoint.split_once(' ')?;
    let mut lines = vec![format!(
        "curl.exe -X {} \"<registry-base-url>{}\"",
        method, path
    )];

    if let Some(header_hint) = &item.header_hint {
        for header in header_hint
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            lines.push(format!("  -H \"{}\"", header));
        }
    }

    if let Some(body_hint) = &item.body_hint {
        lines.push("  -H \"Content-Type: application/json\"".to_string());
        lines.push(format!("  --data-raw '{}'", body_hint));
    }

    Some(lines.join(" \\\n"))
}

fn lifecycle_detail_lines(
    request: Option<&RegistryPublishRequestLifecycle>,
    locale: Locale,
) -> Vec<String> {
    let Some(request) = request else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    if !request.warnings.is_empty() {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Warnings", "Предупреждения"),
            request.warnings.join("; ")
        ));
    }
    if !request.errors.is_empty() {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Errors", "Ошибки"),
            request.errors.join("; ")
        ));
    }
    if let Some(reason) = request
        .rejection_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!(
            "{}: {reason}",
            tr(locale, "Rejection reason", "Причина отклонения")
        ));
    }
    lines
}

fn is_validation_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "validation_queued" | "validation_passed" | "validation_failed"
    )
}

fn latest_validation_event(
    events: &[RegistryGovernanceEventLifecycle],
) -> Option<&RegistryGovernanceEventLifecycle> {
    events
        .iter()
        .find(|event| is_validation_event_type(&event.event_type))
}

fn latest_governance_event_of_types<'a>(
    events: &'a [RegistryGovernanceEventLifecycle],
    event_types: &[&str],
) -> Option<&'a RegistryGovernanceEventLifecycle> {
    events.iter().find(|event| {
        event_types
            .iter()
            .any(|event_type| event.event_type.eq_ignore_ascii_case(event_type))
    })
}

fn registry_request_is_review_ready(request: &RegistryPublishRequestLifecycle) -> bool {
    status_eq(&request.status, "approved")
}

fn registry_validation_outcome_summary(
    request: &RegistryPublishRequestLifecycle,
    events: &[RegistryGovernanceEventLifecycle],
    locale: Locale,
) -> Option<String> {
    let outcome = if status_eq(&request.status, "draft") {
        tr(
            locale,
            "Waiting for artifact upload",
            "Ожидается загрузка артефакта",
        )
        .to_string()
    } else if status_eq(&request.status, "artifact_uploaded")
        || status_eq(&request.status, "submitted")
    {
        tr(
            locale,
            "Artifact uploaded, waiting for validation",
            "Артефакт загружен, ожидается валидация",
        )
        .to_string()
    } else if status_eq(&request.status, "validating") {
        tr(locale, "Validation is running", "Валидация выполняется").to_string()
    } else if status_eq(&request.status, "approved") {
        tr(
            locale,
            "Validation passed; request is ready for governance review",
            "Валидация пройдена; запрос готов к governance-review",
        )
        .to_string()
    } else if status_eq(&request.status, "published") {
        tr(
            locale,
            "Validation passed and the release is already published",
            "Валидация пройдена, релиз уже опубликован",
        )
        .to_string()
    } else if status_eq(&request.status, "rejected") {
        if latest_governance_event_of_types(events, &["validation_failed"]).is_some() {
            tr(
                locale,
                "Validation failed before governance approval",
                "Валидация завершилась ошибкой до governance-approval",
            )
            .to_string()
        } else if latest_governance_event_of_types(events, &["request_rejected"]).is_some()
            || request.rejected_by.is_some()
        {
            tr(
                locale,
                "Request was manually rejected by governance review",
                "Запрос был вручную отклонён на governance-review",
            )
            .to_string()
        } else {
            tr(locale, "Request is rejected", "Запрос отклонён").to_string()
        }
    } else {
        return None;
    };

    Some(outcome)
}

fn follow_up_gate_label(key: &str, locale: Locale) -> String {
    match key {
        "compile_smoke" => tr(locale, "Compile smoke", "Compile smoke").to_string(),
        "targeted_tests" => tr(locale, "Targeted tests", "Targeted tests").to_string(),
        "security_policy_review" => {
            tr(locale, "Security/policy review", "Security/policy review").to_string()
        }
        _ => humanize_token(key),
    }
}

fn registry_review_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> String {
    owner_binding
        .map(|owner| {
            format!(
                "{} / {} / {}",
                registry_review_authority_label(owner_binding, locale),
                tr(locale, "registry admin", "registry admin"),
                tr(locale, "moderator", "moderator")
            )
        })
        .unwrap_or_else(|| {
            format!(
                "{} / {}",
                tr(locale, "registry admin", "registry admin"),
                tr(locale, "moderator", "moderator")
            )
        })
}

fn registry_owner_transfer_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> String {
    owner_binding
        .map(|owner| {
            format!(
                "{} / {}",
                owner.owner_actor,
                tr(locale, "registry admin", "registry admin")
            )
        })
        .unwrap_or_else(|| tr(locale, "registry admin", "registry admin").to_string())
}

fn registry_yank_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    request: Option<&RegistryPublishRequestLifecycle>,
    locale: Locale,
) -> String {
    let mut actors = Vec::new();
    if let Some(owner) = owner_binding {
        actors.push(owner.owner_actor.clone());
    }
    if let Some(release) = release {
        if !actors.iter().any(|actor| actor == &release.publisher) {
            actors.push(release.publisher.clone());
        }
    } else if let Some(request) = request.and_then(|request| request.publisher_identity.clone()) {
        if !actors.iter().any(|actor| actor == &request) {
            actors.push(request);
        }
    }
    actors.push(tr(locale, "registry admin", "registry admin").to_string());
    actors.join(" / ")
}

fn follow_up_gate_status_summary(
    gates: &[RegistryFollowUpGateLifecycle],
    locale: Locale,
) -> Option<String> {
    if gates.is_empty() {
        return None;
    }

    let pending = gates
        .iter()
        .filter(|gate| status_eq(&gate.status, "pending"))
        .count();
    let running = gates
        .iter()
        .filter(|gate| status_eq(&gate.status, "running"))
        .count();
    let passed = gates
        .iter()
        .filter(|gate| status_eq(&gate.status, "passed"))
        .count();
    let failed = gates
        .iter()
        .filter(|gate| status_eq(&gate.status, "failed"))
        .count();
    let blocked = gates
        .iter()
        .filter(|gate| status_eq(&gate.status, "blocked"))
        .count();
    let summary = format!(
        "{}: {} В· {}: {} В· {}: {} В· {}: {} В· {}: {}",
        tr(locale, "Pending", "Р’ РѕР¶РёРґР°РЅРёРё"),
        pending,
        tr(locale, "Running", "В работе"),
        running,
        tr(locale, "Passed", "РџСЂРѕР№РґРµРЅРѕ"),
        passed,
        tr(locale, "Failed", "РџСЂРѕРІР°Р»РµРЅРѕ"),
        failed,
        tr(locale, "Blocked", "Заблокировано"),
        blocked
    );
    return Some(summary);

    /* Some(format!(
        "{}: {} · {}: {} · {}: {}",
        tr(locale, "Pending", "В ожидании"),
        pending,
        tr(locale, "Passed", "Пройдено"),
        passed,
        tr(locale, "Failed", "Провалено"),
        failed
    )) */
}

fn validation_stage_status_summary(
    stages: &[RegistryValidationStageLifecycle],
    locale: Locale,
) -> Option<String> {
    if stages.is_empty() {
        return None;
    }

    let queued = stages
        .iter()
        .filter(|stage| status_eq(&stage.status, "queued"))
        .count();
    let running = stages
        .iter()
        .filter(|stage| status_eq(&stage.status, "running"))
        .count();
    let passed = stages
        .iter()
        .filter(|stage| status_eq(&stage.status, "passed"))
        .count();
    let failed = stages
        .iter()
        .filter(|stage| status_eq(&stage.status, "failed"))
        .count();
    let blocked = stages
        .iter()
        .filter(|stage| status_eq(&stage.status, "blocked"))
        .count();

    Some(format!(
        "{}: {} В· {}: {} В· {}: {} В· {}: {} В· {}: {}",
        tr(locale, "Queued", "В очереди"),
        queued,
        tr(locale, "Running", "В работе"),
        running,
        tr(locale, "Passed", "Пройдено"),
        passed,
        tr(locale, "Failed", "Провалено"),
        failed,
        tr(locale, "Blocked", "Заблокировано"),
        blocked
    ))
}

fn registry_review_policy_lines(
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();

    match owner_binding {
        Some(_) => lines.push(format!(
            "{}: {}",
            tr(locale, "Review authority", "Кто ревьюит"),
            format!(
                "{} / {}",
                owner.owner_actor,
                tr(
                    locale,
                    "governance actor may override",
                    "governance actor может переопределить"
                )
            )
        )),
        None => lines.push(
            tr(
                locale,
                "No persisted owner binding yet; first publish still needs governance/bootstrap handling before review becomes owner-driven.",
                "Сохранённой привязки владельца пока нет; первый publish всё ещё требует governance/bootstrap-обработки, прежде чем review станет owner-driven.",
            )
            .to_string(),
        ),
    }

    lines.push(format!(
        "{}: {}",
        tr(locale, "Owner transfer authority", "Кто меняет владельца"),
        registry_owner_transfer_authority_label(owner_binding, locale)
    ));
    lines.push(format!(
        "{}: {}",
        tr(locale, "Yank authority", "Кто отзывает релиз"),
        registry_yank_authority_label(owner_binding, release, request, locale)
    ));

    if let Some(request) = request {
        match request.status.as_str() {
            status if status_eq(status, "validating") => lines.push(
                tr(
                    locale,
                    "Validation is running asynchronously; wait for APPROVED or REJECTED before any review action.",
                    "Валидация идёт асинхронно; дождитесь APPROVED или REJECTED, прежде чем делать review-действия.",
                )
                .to_string(),
            ),
            status if status_eq(status, "approved") => lines.push(
                tr(
                    locale,
                    "Request is ready for owner/admin/moderator review; requester and publisher identity no longer imply self-review access.",
                    "Запрос готов к governance approval; requester и publisher identity больше не означают право на self-review.",
                )
                .to_string(),
            ),
            status if status_eq(status, "rejected") => lines.push(
                tr(
                    locale,
                    "Rejected requests should be fixed and recreated; moderation stays with the persisted owner or registry review actors.",
                    "Отклонённые запросы нужно исправлять и создавать заново; moderation остаётся у сохранённого владельца или governance actor.",
                )
                .to_string(),
            ),
            status if status_eq(status, "published") => lines.push(
                tr(
                    locale,
                    "Future review actions for this slug now follow the persisted owner binding, not the original publish requester.",
                    "Дальнейшие review-действия для этого slug теперь идут по сохранённой привязке владельца, а не по исходному publish requester.",
                )
                .to_string(),
            ),
            _ => {}
        }

        if owner_binding.is_some()
            && request.publisher_identity.is_some()
            && request.publisher_identity.as_ref() != owner_binding.map(|owner| &owner.owner_actor)
        {
            lines.push(
                tr(
                    locale,
                    "Requested publisher differs from the persisted owner; use owner transfer before treating the new publisher as canonical.",
                    "Запрошенный publisher отличается от сохранённого владельца; сначала выполните owner transfer, прежде чем считать нового publisher каноническим.",
                )
                .to_string(),
            );
        }
    }

    lines
}

fn registry_next_action_lines(
    module: &MarketplaceModule,
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();

    if module.ownership != "first_party" {
        lines.push(
            tr(
                locale,
                "Live publish is still first-party-oriented; keep third-party modules on governance/manual review until the broader moderation flow is finished.",
                "Live publish пока ориентирован на first-party; держите third-party модули на governance/manual review, пока более широкий moderation flow не завершён.",
            )
            .to_string(),
        );
        return lines;
    }

    let xtask_prefix = format!("cargo xtask module");

    match request.map(|request| request.status.as_str()) {
        None => lines.push(format!(
            "{}: {} publish {} --dry-run {}",
            tr(locale, "Start with", "Начните с"),
            xtask_prefix,
            module.slug,
            tr(
                locale,
                "to inspect the publish payload before using a live registry URL.",
                "чтобы проверить publish payload перед live registry URL."
            )
        )),
        Some(status) if status_eq(status, "draft") => lines.push(
            tr(
                locale,
                "Upload the artifact bundle next; review and publish cannot start before artifact upload finishes.",
                "Следующий шаг — загрузка artifact bundle; review и publish не начнутся, пока загрузка не завершится.",
            )
            .to_string(),
        ),
        Some(status) if status_eq(status, "artifact_uploaded") || status_eq(status, "submitted") => lines.push(
            tr(
                locale,
                "Trigger validation next; the request is waiting for the explicit validate step.",
                "Следующий шаг — запуск validation; запрос ждёт явного validate step.",
            )
            .to_string(),
        ),
        Some(status) if status_eq(status, "validating") => lines.push(
            tr(
                locale,
                "Wait for validation to finish and refresh the request status; approve/reject is blocked while the async validator is still running.",
                "Дождитесь завершения validation и обновите статус запроса; approve/reject заблокированы, пока асинхронный validator ещё работает.",
            )
            .to_string(),
        ),
        Some(status) if status_eq(status, "approved") => {
            if let Some(owner) = owner_binding {
                lines.push(format!(
                    "{}: {}.",
                    tr(locale, "Review can now be finalized by", "Review теперь может завершить"),
                    owner.owner_actor
                ));
            } else {
                lines.push(
                    tr(
                        locale,
                        "The request is approved, but there is still no persisted owner binding; governance actor approval remains the safe path.",
                        "Запрос approved, но сохранённой привязки владельца ещё нет; approval через governance actor остаётся безопасным путём.",
                    )
                    .to_string(),
                );
            }
        }
        Some(status) if status_eq(status, "rejected") => lines.push(format!(
            "{}: {} publish {} --dry-run {}",
            tr(locale, "Next step", "Следующий шаг"),
            xtask_prefix,
            module.slug,
            tr(
                locale,
                "after fixing the surfaced errors and rejection reason.",
                "после исправления surfaced errors и причины отклонения."
            )
        )),
        Some(status) if status_eq(status, "published") => lines.push(
            tr(
                locale,
                "The active release is already published; only owner transfer or yank/new version publish should be needed from here.",
                "Активный релиз уже опубликован; дальше обычно нужны только owner transfer или yank/публикация новой версии.",
            )
            .to_string(),
        ),
        Some(_) => {}
    }

    if owner_binding.is_some()
        && request
            .and_then(|request| request.publisher_identity.as_ref())
            .zip(owner_binding.map(|owner| owner.owner_actor.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(format!(
            "{}: {} owner-transfer {} <new-owner-actor> --dry-run {}",
            tr(
                locale,
                "If ownership should move",
                "Если владение должно перейти"
            ),
            xtask_prefix,
            module.slug,
            tr(
                locale,
                "before treating the requested publisher as canonical.",
                "прежде чем считать requested publisher каноническим."
            )
        ));
    }

    if release.is_some_and(|release| status_eq(&release.status, "yanked")) {
        lines.push(
            tr(
                locale,
                "Latest release is yanked; publish a fresh active version instead of expecting the catalog to recover automatically.",
                "Последний релиз отозван; публикуйте новую active-версию, а не ждите, что каталог восстановится автоматически.",
            )
            .to_string(),
        );
    }

    lines
}

fn registry_operator_command_lines(
    module: &MarketplaceModule,
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    validation_stages: &[RegistryValidationStageLifecycle],
) -> Vec<String> {
    let mut lines = Vec::new();

    if module.ownership != "first_party" {
        return lines;
    }

    let publish_dry_run = format!("cargo xtask module publish {} --dry-run", module.slug);
    let publish_live = format!(
        "cargo xtask module publish {} --registry-url <registry-url>",
        module.slug
    );

    match request.map(|request| request.status.as_str()) {
        None => lines.push(publish_dry_run.clone()),
        Some(status) if status_eq(status, "draft") => lines.push(publish_live),
        Some(status) if status_eq(status, "rejected") => lines.push(publish_dry_run.clone()),
        Some(status) if status_eq(status, "published") => {
            let version = release
                .map(|release| release.version.clone())
                .unwrap_or_else(|| module.latest_version.clone());
            lines.push(format!(
                "cargo xtask module yank {} {} --dry-run",
                module.slug, version
            ));
        }
        _ => {}
    }

    if owner_binding.is_some()
        && request
            .and_then(|request| request.publisher_identity.as_ref())
            .zip(owner_binding.map(|owner| owner.owner_actor.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(format!(
            "cargo xtask module owner-transfer {} <new-owner-actor> --dry-run",
            module.slug
        ));
    }

    if release.is_some_and(|release| status_eq(&release.status, "yanked")) {
        lines.push(publish_dry_run);
    }

    if let Some(request) = request {
        if !validation_stages.is_empty()
            && (status_eq(&request.status, "approved") || status_eq(&request.status, "published"))
        {
            for stage in validation_stages {
                lines.push(format!(
                    "cargo xtask module stage {} {} <queued|running|passed|failed|blocked> --dry-run",
                    request.id, stage.key
                ));
            }
        }
    }

    lines.sort();
    lines.dedup();
    lines
}

fn registry_live_api_action_lines(
    module: &MarketplaceModule,
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    validation_stages: &[RegistryValidationStageLifecycle],
    locale: Locale,
) -> Vec<RegistryLiveApiActionHint> {
    let Some(request) = request else {
        return Vec::new();
    };

    let owner_actor = owner_binding.map(|owner| owner.owner_actor.as_str());
    let request_actor = request.requested_by.as_str();
    let publisher_identity = request.publisher_identity.as_deref();
    let actor_header_hint = |prefer_owner: bool| {
        if prefer_owner {
            owner_actor
                .map(|owner| format!("x-rustok-actor: {}", owner))
                .unwrap_or_else(|| format!("x-rustok-actor: {}", request_actor))
        } else {
            format!("x-rustok-actor: {}", request_actor)
        }
    };
    let publisher_header_hint =
        || publisher_identity.map(|publisher| format!("x-rustok-publisher: {}", publisher));

    let mut lines = vec![RegistryLiveApiActionHint {
        endpoint: format!("GET /v2/catalog/publish/{}", request.id),
        authority: tr(
            locale,
            "Any operator with registry access",
            "Любой оператор с доступом к registry",
        )
        .to_string(),
        note: Some(
            tr(
                locale,
                "Read-only status lookup for the current publish request.",
                "Read-only просмотр статуса для текущего publish request.",
            )
            .to_string(),
        ),
        body_hint: None,
        header_hint: None,
        xtask_hint: None,
        write_path: false,
    }];

    if status_eq(&request.status, "artifact_uploaded") || status_eq(&request.status, "submitted") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/validate", request.id),
            authority: owner_actor
                .map(|owner| format!("{} / governance", owner))
                .unwrap_or_else(|| {
                    format!(
                        "{}{}{}",
                        request_actor,
                        publisher_identity
                            .map(|publisher| format!(" / {}", publisher))
                            .unwrap_or_default(),
                        " / governance"
                    )
                }),
            note: Some(
                tr(
                    locale,
                    "Validation starts the async review gate after artifact upload.",
                    "Validation запускает асинхронный review gate после загрузки артефакта.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(actor_header_hint(owner_binding.is_some())),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url>",
                module.slug
            )),
            write_path: true,
        });
    }

    if status_eq(&request.status, "approved") {
        let review_authority = registry_review_authority_label(owner_binding, locale);
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/approve", request.id),
            authority: review_authority.clone(),
            note: Some(
                tr(
                    locale,
                    "Finalize a validated request into a published release.",
                    "Финализирует провалидированный запрос в опубликованный релиз.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(
                std::iter::once(actor_header_hint(true))
                    .chain(publisher_header_hint())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url>",
                module.slug
            )),
            write_path: true,
        });
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/reject", request.id),
            authority: review_authority,
            note: Some(
                tr(
                    locale,
                    "Reject requires a governance reason in the request body.",
                    "Reject требует governance reason в теле запроса.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<governance-reason>\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<governance-reason>\" }",
                )
                .to_string(),
            ),
            header_hint: Some(actor_header_hint(true)),
            xtask_hint: None,
            write_path: true,
        });
        if let Some(stage) = validation_stages.first() {
            lines.push(RegistryLiveApiActionHint {
                endpoint: format!("POST /v2/catalog/publish/{}/stages", request.id),
                authority: registry_review_authority_label(owner_binding, locale),
                note: Some(
                    tr(
                        locale,
                        "Persist external follow-up validation stage state without changing publish approval semantics.",
                        "Сохранить состояние внешнего follow-up validation stage без изменения publish approval semantics.",
                    )
                    .to_string(),
                ),
                body_hint: Some(format!(
                    "{{ \"schema_version\": 1, \"dry_run\": false, \"stage\": \"{}\", \"status\": \"passed\", \"detail\": \"External validation recorded by operator.\", \"requeue\": false }}",
                    stage.key
                )),
                header_hint: Some(actor_header_hint(true)),
                xtask_hint: Some(format!(
                    "cargo xtask module stage {} {} passed --detail \"External validation recorded by operator.\" --registry-url <registry-url>",
                    request.id, stage.key
                )),
                write_path: true,
            });
        }
    } else if status_eq(&request.status, "validating") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("GET /v2/catalog/publish/{}", request.id),
            authority: tr(
                locale,
                "Any operator with registry access",
                "Любой оператор с доступом к registry",
            )
            .to_string(),
            note: Some(
                tr(
                    locale,
                    "Poll until validation leaves the validating state.",
                    "Проверяйте статус, пока validation не выйдет из validating.",
                )
                .to_string(),
            ),
            body_hint: None,
            header_hint: None,
            xtask_hint: None,
            write_path: false,
        });
    } else if status_eq(&request.status, "published") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/yank".to_string(),
            authority: registry_yank_authority_label(owner_binding, release, Some(request), locale),
            note: Some(
                tr(
                    locale,
                    "Yank acts on the published release trail, not on the request.",
                    "Yank работает по опубликованному release trail, а не по самому request.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"version\": \"<version>\", \"reason\": \"<yank-reason>\", \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"version\": \"<version>\", \"reason\": \"<yank-reason>\", \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(actor_header_hint(owner_binding.is_some())),
            xtask_hint: Some(format!(
                "cargo xtask module yank {} {} --reason <yank-reason> --registry-url <registry-url>",
                module.slug,
                release
                    .map(|value| value.version.as_str())
                    .unwrap_or(module.latest_version.as_str())
            )),
            write_path: true,
        });
    } else if status_eq(&request.status, "rejected") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/publish".to_string(),
            authority: owner_actor
                .map(|owner| format!("{} / governance", owner))
                .unwrap_or_else(|| {
                    format!(
                        "{}{}{}",
                        request_actor,
                        publisher_identity
                            .map(|publisher| format!(" / {}", publisher))
                            .unwrap_or_default(),
                        " / governance"
                    )
                }),
            note: Some(
                tr(
                    locale,
                    "Rejected requests are recreated, not reopened in place.",
                    "Rejected requests создаются заново, а не переоткрываются на месте.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"module\": { ... } }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"module\": { ... } }",
                )
                .to_string(),
            ),
            header_hint: Some(
                std::iter::once(actor_header_hint(owner_binding.is_some()))
                    .chain(publisher_header_hint())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url>",
                module.slug
            )),
            write_path: true,
        });
    }

    if owner_binding.is_some()
        && request
            .publisher_identity
            .as_ref()
            .zip(owner_binding.map(|owner| owner.owner_actor.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/owner-transfer".to_string(),
            authority: registry_owner_transfer_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Use this before treating a new requested publisher as the canonical owner.",
                    "Используйте это до того, как считать нового requested publisher каноническим владельцем.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"new_owner_actor\": \"<actor>\", \"reason\": \"<transfer-reason>\", \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"new_owner_actor\": \"<actor>\", \"reason\": \"<transfer-reason>\", \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(actor_header_hint(true)),
            xtask_hint: Some(format!(
                "cargo xtask module owner-transfer {} <new-owner-actor> --reason <transfer-reason> --registry-url <registry-url>",
                module.slug
            )),
            write_path: true,
        });
    }

    if release.is_some_and(|release| status_eq(&release.status, "yanked")) {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/publish".to_string(),
            authority: owner_actor
                .map(|owner| format!("{} / governance", owner))
                .unwrap_or_else(|| {
                    format!(
                        "{}{}{}",
                        request_actor,
                        publisher_identity
                            .map(|publisher| format!(" / {}", publisher))
                            .unwrap_or_default(),
                        " / governance"
                    )
                }),
            note: Some(
                tr(
                    locale,
                    "A yanked release recovers through a fresh publish request.",
                    "Yanked release восстанавливается через новый publish request.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"module\": { ... } }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"module\": { ... } }",
                )
                .to_string(),
            ),
            header_hint: Some(
                std::iter::once(actor_header_hint(owner_binding.is_some()))
                    .chain(publisher_header_hint())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url>",
                module.slug
            )),
            write_path: true,
        });
    }

    lines.sort_by(|left, right| left.endpoint.cmp(&right.endpoint));
    lines.dedup_by(|left, right| left.endpoint == right.endpoint);
    lines
}

fn governance_detail_string(details: &serde_json::Value, key: &str) -> Option<String> {
    details
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn governance_detail_string_list(details: &serde_json::Value, key: &str) -> Vec<String> {
    details
        .get(key)
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
        .filter(|value| !value.is_empty())
        .collect()
}

fn governance_detail_i64(details: &serde_json::Value, key: &str) -> Option<i64> {
    details.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|number| number as i64))
    })
}

fn governance_event_stage_key(event: &RegistryGovernanceEventLifecycle) -> Option<String> {
    governance_detail_string(&event.details, "stage")
        .or_else(|| governance_detail_string(&event.details, "gate"))
}

fn validation_stage_recent_history(
    events: &[RegistryGovernanceEventLifecycle],
    stage_key: &str,
    limit: usize,
) -> Vec<RegistryGovernanceEventLifecycle> {
    events
        .iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "validation_stage_queued"
                    | "validation_stage_running"
                    | "validation_stage_started"
                    | "validation_stage_passed"
                    | "validation_stage_failed"
                    | "validation_stage_blocked"
                    | "follow_up_gate_queued"
                    | "follow_up_gate_passed"
                    | "follow_up_gate_failed"
            ) && governance_event_stage_key(event)
                .as_deref()
                .is_some_and(|value| value == stage_key)
        })
        .take(limit)
        .cloned()
        .collect()
}

fn governance_event_title(event_type: &str, locale: Locale) -> String {
    let event_type = match event_type {
        "validation_stage_started" => "validation_stage_running",
        other => other,
    };
    match event_type {
        "request_created" => tr(
            locale,
            "Publish request created",
            "Создан запрос на публикацию",
        ),
        "artifact_uploaded" => tr(locale, "Artifact uploaded", "Артефакт загружен"),
        "validation_queued" => tr(
            locale,
            "Validation queued",
            "Валидация поставлена в очередь",
        ),
        "validation_passed" => tr(locale, "Validation passed", "Валидация пройдена"),
        "validation_failed" => tr(locale, "Validation failed", "Валидация провалена"),
        "release_published" => tr(locale, "Release published", "Релиз опубликован"),
        "request_rejected" => tr(locale, "Request rejected", "Запрос отклонён"),
        "release_yanked" => tr(locale, "Release yanked", "Релиз отозван"),
        "owner_bound" => tr(
            locale,
            "Owner binding updated",
            "Связка владельца обновлена",
        ),
        "owner_transferred" => tr(locale, "Owner transferred", "Владелец передан"),
        "validation_stage_queued" => tr(
            locale,
            "Validation stage queued",
            "Этап валидации поставлен в очередь",
        ),
        "validation_stage_running" | "validation_stage_started" => tr(
            locale,
            "Validation stage running",
            "Этап валидации выполняется",
        ),
        "validation_stage_passed" => {
            tr(locale, "Validation stage passed", "Этап валидации пройден")
        }
        "validation_stage_failed" => {
            tr(locale, "Validation stage failed", "Этап валидации провален")
        }
        "validation_stage_blocked" => tr(
            locale,
            "Validation stage blocked",
            "Этап валидации заблокирован",
        ),
        "follow_up_gate_queued" => tr(
            locale,
            "Follow-up gate queued",
            "Внешний gate поставлен в очередь",
        ),
        "follow_up_gate_passed" => tr(locale, "Follow-up gate passed", "Внешний gate пройден"),
        "follow_up_gate_failed" => tr(locale, "Follow-up gate failed", "Внешний gate провален"),
        _ => return humanize_token(event_type),
    }
    .to_string()
}

fn governance_event_summary(event: &RegistryGovernanceEventLifecycle, locale: Locale) -> String {
    let event_type = match event.event_type.as_str() {
        "validation_stage_started" => "validation_stage_running",
        other => other,
    };
    let version = governance_detail_string(&event.details, "version");
    let reason = governance_detail_string(&event.details, "reason");
    let publisher =
        governance_detail_string(&event.details, "publisher").or_else(|| event.publisher.clone());
    let owner_actor = governance_detail_string(&event.details, "owner_actor");
    let mode = governance_detail_string(&event.details, "mode");
    let warnings = governance_detail_string_list(&event.details, "warnings");
    let errors = governance_detail_string_list(&event.details, "errors");
    let stage_key = governance_event_stage_key(event);
    let stage_label = stage_key
        .as_deref()
        .map(|value| follow_up_gate_label(value, locale))
        .unwrap_or_else(|| tr(locale, "Validation stage", "Этап валидации").to_string());
    let stage_attempt = governance_detail_i64(&event.details, "attempt_number");
    let stage_detail = governance_detail_string(&event.details, "detail");

    match event_type {
        "request_created" => version
            .map(|value| {
                format!(
                    "{} v{}",
                    tr(
                        locale,
                        "Version queued for publish",
                        "Версия поставлена в очередь на публикацию"
                    ),
                    value
                )
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Publish request was created.",
                    "Запрос на публикацию создан.",
                )
                .to_string()
            }),
        "artifact_uploaded" => version
            .map(|value| {
                format!(
                    "{} v{}",
                    tr(
                        locale,
                        "Artifact stored for version",
                        "Артефакт сохранён для версии"
                    ),
                    value
                )
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Artifact stored and ready for validation.",
                    "Артефакт сохранён и готов к валидации.",
                )
                .to_string()
            }),
        "validation_queued" => tr(
            locale,
            "Validation job was queued; poll the request status for completion.",
            "Задача валидации поставлена в очередь; следите за статусом запроса.",
        )
        .to_string(),
        "validation_stage_queued"
        | "validation_stage_running"
        | "validation_stage_started"
        | "validation_stage_passed"
        | "validation_stage_failed"
        | "validation_stage_blocked" => {
            let status = match event_type {
                "validation_stage_queued" => tr(
                    locale,
                    "queued for operator follow-up",
                    "поставлен в очередь для оператора",
                ),
                "validation_stage_running" => tr(locale, "is running", "выполняется"),
                "validation_stage_passed" => tr(locale, "passed", "пройден"),
                "validation_stage_failed" => tr(locale, "failed", "провален"),
                "validation_stage_blocked" => tr(locale, "is blocked", "заблокирован"),
                _ => unreachable!(),
            };

            let mut parts = vec![format!("{stage_label} {status}")];
            if let Some(attempt) = stage_attempt {
                parts.push(format!("{} {}", tr(locale, "attempt", "попытка"), attempt));
            }
            if let Some(detail) = stage_detail {
                parts.push(detail);
            }
            parts.join(" · ")
        }
        "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed" => {
            let status = match event_type {
                "follow_up_gate_queued" => tr(
                    locale,
                    "queued for external follow-up",
                    "поставлен в очередь для внешнего gate",
                ),
                "follow_up_gate_passed" => tr(locale, "passed", "пройден"),
                "follow_up_gate_failed" => tr(locale, "failed", "провален"),
                _ => unreachable!(),
            };

            let mut parts = vec![format!("{stage_label} {status}")];
            if let Some(detail) = stage_detail {
                parts.push(detail);
            }
            parts.join(" · ")
        }
        "validation_passed" => {
            if warnings.is_empty() {
                tr(
                    locale,
                    "Validation completed without blocking errors.",
                    "Валидация завершилась без блокирующих ошибок.",
                )
                .to_string()
            } else {
                format!(
                    "{}: {}",
                    tr(
                        locale,
                        "Validation passed with warnings",
                        "Валидация пройдена с предупреждениями"
                    ),
                    warnings.join("; ")
                )
            }
        }
        "validation_failed" => reason
            .map(|value| {
                format!(
                    "{}: {}",
                    tr(locale, "Validation failed", "Валидация провалена"),
                    value
                )
            })
            .or_else(|| {
                (!errors.is_empty()).then(|| {
                    format!(
                        "{}: {}",
                        tr(locale, "Validation errors", "Ошибки валидации"),
                        errors.join("; ")
                    )
                })
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Validation failed and requires follow-up.",
                    "Валидация провалена и требует доработки.",
                )
                .to_string()
            }),
        "release_published" => {
            let version_part = version
                .map(|value| format!("v{value}"))
                .unwrap_or_else(|| tr(locale, "new version", "новая версия").to_string());
            match publisher {
                Some(publisher) => format!(
                    "{} {} ({})",
                    tr(locale, "Published", "Опубликован"),
                    version_part,
                    publisher
                ),
                None => format!(
                    "{} {}",
                    tr(locale, "Published", "Опубликован"),
                    version_part
                ),
            }
        }
        "request_rejected" => reason
            .map(|value| format!("{}: {}", tr(locale, "Rejected", "Отклонён"), value))
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Request was rejected by governance policy.",
                    "Запрос отклонён по governance policy.",
                )
                .to_string()
            }),
        "release_yanked" => reason
            .map(|value| format!("{}: {}", tr(locale, "Yanked", "Отозван"), value))
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Release was yanked from the active catalog.",
                    "Релиз отозван из активного каталога.",
                )
                .to_string()
            }),
        "owner_bound" => {
            let label = match mode.as_deref() {
                Some("rebind") => tr(locale, "Owner rebound", "Владелец перевязан"),
                _ => tr(locale, "Owner bound", "Владелец привязан"),
            };
            owner_actor
                .map(|owner_actor| format!("{label}: {owner_actor}"))
                .unwrap_or_else(|| label.to_string())
        }
        "owner_transferred" => {
            let previous_owner = governance_detail_string(&event.details, "previous_owner_actor");
            let new_owner =
                governance_detail_string(&event.details, "new_owner_actor").or(owner_actor);
            match (previous_owner, new_owner, reason) {
                (Some(previous_owner), Some(new_owner), Some(reason)) => format!(
                    "{}: {} -> {} ({})",
                    tr(locale, "Ownership transferred", "Владение передано"),
                    previous_owner,
                    new_owner,
                    reason
                ),
                (Some(previous_owner), Some(new_owner), None) => format!(
                    "{}: {} -> {}",
                    tr(locale, "Ownership transferred", "Владение передано"),
                    previous_owner,
                    new_owner
                ),
                (_, Some(new_owner), Some(reason)) => format!(
                    "{}: {} ({})",
                    tr(locale, "New owner", "Новый владелец"),
                    new_owner,
                    reason
                ),
                (_, Some(new_owner), None) => format!(
                    "{}: {}",
                    tr(locale, "New owner", "Новый владелец"),
                    new_owner
                ),
                _ => tr(
                    locale,
                    "Persisted owner binding was transferred to a new actor.",
                    "Сохранённая привязка владельца передана новому актору.",
                )
                .to_string(),
            }
        }
        _ => humanize_token(&event.event_type),
    }
}

fn humanize_token(value: &str) -> String {
    value
        .split(['-', '_'])
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

fn humanize_setting_key(value: &str) -> String {
    let mut rendered = String::new();
    let mut previous_was_lowercase = false;

    for ch in value.chars() {
        if (ch == '_' || ch == '-') && !rendered.ends_with(' ') {
            rendered.push(' ');
            previous_was_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_was_lowercase && !rendered.ends_with(' ') {
            rendered.push(' ');
        }

        rendered.push(ch);
        previous_was_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    humanize_token(rendered.trim())
}

fn setting_field_hint(field: &ModuleSettingField, locale: Locale) -> Option<String> {
    let mut parts = Vec::new();
    if field.required {
        parts.push(tr(locale, "Required", "Обязательно").to_string());
    }
    if let Some(default) = &field.default_value {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Default", "По умолчанию"),
            default
        ));
    }
    match (field.min, field.max) {
        (Some(min), Some(max)) => parts.push(format!(
            "{}: {}..{}",
            tr(locale, "Range", "Диапазон"),
            min,
            max
        )),
        (Some(min), None) => parts.push(format!("{}: {}", tr(locale, "Min", "Минимум"), min)),
        (None, Some(max)) => parts.push(format!("{}: {}", tr(locale, "Max", "Максимум"), max)),
        (None, None) => {}
    }
    if !field.options.is_empty() {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Options", "Опции"),
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
            tr(locale, "Object keys", "Ключи объекта"),
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
            tr(locale, "Array items", "Элементы массива"),
            humanize_token(item_type)
        ));
    }

    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn setting_field_placeholder(field: &ModuleSettingField) -> Option<&'static str> {
    match field.value_type.as_str() {
        "object" => Some("{\n  \"key\": \"value\"\n}"),
        "array" => Some("[\n  \"item\"\n]"),
        "json" | "any" => Some("{\n  \"any\": true\n}"),
        _ => None,
    }
}

fn setting_option_draft_value(value_type: &str, value: &serde_json::Value) -> String {
    match value_type {
        "string" => value.as_str().unwrap_or_default().to_string(),
        "integer" => value
            .as_i64()
            .map(|number| number.to_string())
            .or_else(|| value.as_u64().map(|number| number.to_string()))
            .unwrap_or_else(|| value.to_string()),
        "number" => value
            .as_f64()
            .map(|number| {
                let mut rendered = number.to_string();
                if rendered.ends_with(".0") {
                    rendered.truncate(rendered.len() - 2);
                }
                rendered
            })
            .unwrap_or_else(|| value.to_string()),
        "boolean" => value
            .as_bool()
            .map(|flag| flag.to_string())
            .unwrap_or_else(|| value.to_string()),
        _ => value.to_string(),
    }
}

fn setting_option_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

fn setting_shape_properties(shape: Option<&serde_json::Value>) -> Vec<(String, serde_json::Value)> {
    let Some(shape) = shape else {
        return Vec::new();
    };
    let Some(properties) = shape.get("properties").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut entries = properties
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn setting_shape_items(shape: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    shape.and_then(|shape| shape.get("items")).cloned()
}

fn setting_shape_property(
    shape: Option<&serde_json::Value>,
    key: &str,
) -> Option<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("properties"))
        .and_then(|value| value.as_object())
        .and_then(|properties| properties.get(key))
        .cloned()
}

fn setting_shape_type(shape: Option<&serde_json::Value>) -> Option<String> {
    shape
        .and_then(|shape| shape.get("type"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn setting_shape_options(shape: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("options"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

fn setting_shape_numeric_bound(shape: Option<&serde_json::Value>, key: &str) -> Option<String> {
    let value = shape.and_then(|shape| shape.get(key))?;

    value
        .as_i64()
        .map(|number| number.to_string())
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| {
            value.as_f64().map(|number| {
                let mut rendered = number.to_string();
                if rendered.ends_with(".0") {
                    rendered.truncate(rendered.len() - 2);
                }
                rendered
            })
        })
}

fn parse_scalar_input_value(raw: &str, value_type: &str) -> Option<serde_json::Value> {
    match value_type {
        "string" => Some(serde_json::Value::String(raw.to_string())),
        "boolean" => raw.parse::<bool>().ok().map(serde_json::Value::Bool),
        "integer" => raw
            .parse::<i64>()
            .ok()
            .map(|number| serde_json::Value::Number(number.into()))
            .or_else(|| {
                raw.parse::<u64>()
                    .ok()
                    .map(|number| serde_json::Value::Number(number.into()))
            }),
        "number" => raw
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        _ => None,
    }
}

fn render_scalar_value_editor(
    current_value: serde_json::Value,
    shape: Option<serde_json::Value>,
    locale: Locale,
    #[allow(unused_variables)] disabled: Signal<bool>,
    on_input: Callback<serde_json::Value>,
) -> AnyView {
    let value_type = setting_shape_type(shape.as_ref())
        .unwrap_or_else(|| json_value_kind(&current_value).to_string());
    let options = setting_shape_options(shape.as_ref());
    let current_raw = setting_option_draft_value(&value_type, &current_value);
    let min = setting_shape_numeric_bound(shape.as_ref(), "min");
    let max = setting_shape_numeric_bound(shape.as_ref(), "max");

    match value_type.as_str() {
        "boolean" if options.is_empty() => {
            let checked = current_value.as_bool().unwrap_or(false);
            view! {
                <label class="inline-flex items-center gap-3 text-sm text-card-foreground">
                    <input
                        type="checkbox"
                        class="h-4 w-4 rounded border-border text-primary focus:ring-primary/20"
                        checked=checked
                        disabled=move || disabled.get()
                        on:change=move |event| {
                            on_input.run(serde_json::Value::Bool(event_target_checked(&event)))
                        }
                    />
                    <span>{tr(locale, "Enabled", "Включено")}</span>
                </label>
            }
            .into_any()
        }
        "string" | "integer" | "number" | "boolean" if !options.is_empty() => {
            let options_for_select = options.clone();
            let value_type_for_select = value_type.clone();
            view! {
                <select
                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                    prop:value=current_raw.clone()
                    disabled=move || disabled.get()
                    on:change=move |event| {
                        if let Some(next_value) = parse_scalar_input_value(
                            &event_target_value(&event),
                            &value_type_for_select,
                        ) {
                            on_input.run(next_value);
                        }
                    }
                >
                    {options_for_select.into_iter().map(|option| {
                        let option_value = setting_option_draft_value(&value_type, &option);
                        let option_label = setting_option_label(&option);
                        view! {
                            <option value=option_value>{option_label}</option>
                        }
                    }).collect_view()}
                </select>
            }
            .into_any()
        }
        "integer" | "number" => {
            let step = if value_type == "integer" { "1" } else { "any" };
            let value_type_for_input = value_type.clone();
            view! {
                <input
                    type="number"
                    step=step
                    min=min
                    max=max
                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                    value=current_raw
                    disabled=move || disabled.get()
                    on:input=move |event| {
                        if let Some(next_value) = parse_scalar_input_value(
                            &event_target_value(&event),
                            &value_type_for_input,
                        ) {
                            on_input.run(next_value);
                        }
                    }
                />
            }
            .into_any()
        }
        _ => view! {
            <input
                type="text"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                value=current_raw
                disabled=move || disabled.get()
                on:input=move |event| {
                    on_input.run(serde_json::Value::String(event_target_value(&event)))
                }
            />
        }
        .into_any(),
    }
}

fn default_value_for_schema_shape(shape: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(shape) = shape else {
        return serde_json::Value::Null;
    };

    if let Some(default) = shape.get("default") {
        return default.clone();
    }

    match setting_shape_type(Some(shape)).as_deref() {
        Some("object") => {
            let object = setting_shape_properties(Some(shape))
                .into_iter()
                .map(|(key, property_shape)| {
                    (key, default_value_for_schema_shape(Some(&property_shape)))
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::Value::Object(object)
        }
        Some("array") => serde_json::json!([]),
        Some(value_type) => default_value_for_setting_type(value_type),
        None => serde_json::Value::Null,
    }
}

fn schema_action_label(shape: Option<&serde_json::Value>, locale: Locale) -> String {
    match setting_shape_type(shape).as_deref() {
        Some(value_type) => add_item_button_label(value_type, locale),
        None => tr(locale, "Add item", "Добавить элемент").to_string(),
    }
}

fn pretty_json_value(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn parse_json_editor_value(
    raw: &str,
    expected_type: &str,
    locale: Locale,
) -> Result<Option<serde_json::Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|err| format!("{}: {err}", tr(locale, "Invalid JSON", "Некорректный JSON")))?;

    match expected_type {
        "object" if !value.is_object() => {
            Err(tr(locale, "Expected a JSON object", "Ожидался JSON-объект").to_string())
        }
        "array" if !value.is_array() => {
            Err(tr(locale, "Expected a JSON array", "Ожидался JSON-массив").to_string())
        }
        _ => Ok(Some(value)),
    }
}

fn reset_json_editor_value(field_type: &str) -> String {
    let value = match field_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        "json" | "any" => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    };
    pretty_json_value(&value)
}

fn append_object_property(raw: &str) -> Result<String, String> {
    let mut object = match parse_json_editor_value(raw, "object", Locale::en)? {
        Some(serde_json::Value::Object(object)) => object,
        Some(_) => return Err("Expected a JSON object".to_string()),
        None => serde_json::Map::new(),
    };

    let mut next_index = 1;
    let key = loop {
        let candidate = if next_index == 1 {
            "newKey".to_string()
        } else {
            format!("newKey{}", next_index)
        };
        if !object.contains_key(&candidate) {
            break candidate;
        }
        next_index += 1;
    };
    object.insert(key, serde_json::Value::String(String::new()));
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn append_array_item(raw: &str) -> Result<String, String> {
    let mut array = match parse_json_editor_value(raw, "array", Locale::en)? {
        Some(serde_json::Value::Array(array)) => array,
        Some(_) => return Err("Expected a JSON array".to_string()),
        None => Vec::new(),
    };
    array.push(serde_json::Value::Null);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn json_editor_summary(field_type: &str, raw: &str, locale: Locale) -> (bool, String, Vec<String>) {
    match parse_json_editor_value(raw, field_type, locale) {
        Ok(Some(serde_json::Value::Object(object))) => {
            let preview = object.keys().take(4).cloned().collect::<Vec<_>>();
            (
                true,
                format!("{} {}", object.len(), tr(locale, "keys", "ключей")),
                preview,
            )
        }
        Ok(Some(serde_json::Value::Array(array))) => {
            let preview = array
                .iter()
                .take(4)
                .map(|item| match item {
                    serde_json::Value::Null => "null".to_string(),
                    serde_json::Value::Bool(_) => "bool".to_string(),
                    serde_json::Value::Number(_) => "number".to_string(),
                    serde_json::Value::String(_) => "string".to_string(),
                    serde_json::Value::Array(_) => "array".to_string(),
                    serde_json::Value::Object(_) => "object".to_string(),
                })
                .collect::<Vec<_>>();
            (
                true,
                format!("{} {}", array.len(), tr(locale, "items", "элементов")),
                preview,
            )
        }
        Ok(Some(value)) => (
            true,
            format!("{} {}", value, tr(locale, "value ready", "значение готово")),
            Vec::new(),
        ),
        Ok(None) => (
            true,
            tr(
                locale,
                "Empty value; server defaults apply if declared.",
                "Пустое значение; серверные значения по умолчанию применятся, если они объявлены.",
            )
            .to_string(),
            Vec::new(),
        ),
        Err(message) => (false, message, Vec::new()),
    }
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn metadata_status_badge_classes(state: &str) -> &'static str {
    match state {
        "ready" => {
            "inline-flex items-center rounded-full border border-emerald-500/40 bg-emerald-500/10 px-2 py-0.5 font-medium text-emerald-700"
        }
        "warn" => {
            "inline-flex items-center rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 font-medium text-amber-700"
        }
        _ => {
            "inline-flex items-center rounded-full border border-border px-2 py-0.5 font-medium text-muted-foreground"
        }
    }
}

fn metadata_status_panel_classes(state: &str) -> &'static str {
    match state {
        "ready" => "border-emerald-500/30 bg-emerald-500/5",
        "warn" => "border-amber-500/30 bg-amber-500/5",
        _ => "border-border bg-background",
    }
}

fn looks_like_absolute_http_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("https://") || value.starts_with("http://")
}

fn asset_path_without_query(value: &str) -> &str {
    value.split(['?', '#']).next().unwrap_or(value)
}

fn looks_like_svg_url(value: &str) -> bool {
    looks_like_absolute_http_url(value) && asset_path_without_query(value).ends_with(".svg")
}

fn looks_like_image_url(value: &str) -> bool {
    if !looks_like_absolute_http_url(value) {
        return false;
    }

    let lower = asset_path_without_query(value).to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".svg"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn marketplace_metadata_checklist(
    module: &MarketplaceModule,
    locale: Locale,
) -> Vec<MetadataChecklistItem> {
    let description_length = module.description.trim().chars().count();
    let icon_url = module
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let banner_url = module
        .banner_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let screenshots_count = module
        .screenshots
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .count();
    let publisher = module
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let latest_release = latest_active_registry_version(module).cloned();
    let latest_release_version = latest_release
        .as_ref()
        .map(|version| version.version.as_str());
    let latest_release_date = latest_release
        .as_ref()
        .and_then(|version| version.published_at.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let has_yanked_only_versions = !module.versions.is_empty() && latest_release.is_none();
    let has_registry_publish_signal = module.checksum_sha256.is_some() || latest_release.is_some();

    vec![
        if description_length >= 20 {
            MetadataChecklistItem {
                label: tr(locale, "Description", "Описание"),
                state: "ready",
                priority: "required",
                summary: tr(locale, "Ready", "Готово"),
                detail: format!(
                    "{} {}",
                    description_length,
                    tr(
                        locale,
                        "characters available for catalog detail.",
                        "символов доступно для карточки каталога.",
                    )
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Description", "Описание"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Needs at least 20 characters to satisfy local manifest validation.",
                    "Нужно минимум 20 символов, чтобы пройти локальную валидацию manifest.",
                )
                .to_string(),
            }
        },
        match icon_url {
            Some(value) if looks_like_svg_url(value) => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: tr(
                    locale,
                    "Absolute SVG icon is present for registry cards and detail previews.",
                    "Абсолютный SVG-URL иконки задан для карточек registry и detail preview.",
                )
                .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Icon URL should be an absolute http(s) SVG asset.",
                    "URL иконки должен быть абсолютным http(s) SVG-ресурсом.",
                )
                .to_string(),
            },
            None => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail: tr(
                    locale,
                    "Add an SVG icon URL so registry lists and cards have a visual identity.",
                    "Добавьте SVG-URL иконки, чтобы у карточек и списков registry была визуальная идентичность.",
                )
                .to_string(),
            },
        },
        match banner_url {
            Some(value) if looks_like_image_url(value) => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: tr(
                    locale,
                    "Banner image is present for richer marketplace detail layouts.",
                    "Изображение баннера доступно для более богатого detail layout в marketplace.",
                )
                .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Banner URL should be an absolute http(s) image asset.",
                    "URL баннера должен быть абсолютным http(s) image-ресурсом.",
                )
                .to_string(),
            },
            None => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail:
                    tr(
                        locale,
                        "Optional for local validation, but useful for richer registry presentation.",
                        "Для локальной валидации необязательно, но полезно для richer presentation в registry.",
                    )
                    .to_string(),
            },
        },
        if screenshots_count > 0 {
            MetadataChecklistItem {
                label: tr(locale, "Screenshots", "Скриншоты"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: format!(
                    "{} {}",
                    screenshots_count,
                    tr(locale, "screenshot(s) available for discovery UX.", "скриншотов доступно для discovery UX.")
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Screenshots", "Скриншоты"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail:
                    tr(
                        locale,
                        "Add one or more screenshots to make module capabilities easier to evaluate.",
                        "Добавьте один или несколько скриншотов, чтобы возможности модуля было проще оценивать.",
                    )
                    .to_string(),
            }
        },
        if let Some(publisher) = publisher {
            MetadataChecklistItem {
                label: tr(locale, "Publisher identity", "Идентичность издателя"),
                state: "ready",
                priority: "info",
                summary: tr(locale, "Known", "Известен"),
                detail: format!(
                    "{} {publisher}.",
                    tr(locale, "Publisher is exposed as", "Издатель указан как")
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Publisher identity", "Идентичность издателя"),
                state: "info",
                priority: "info",
                summary: tr(locale, "Local only", "Только локально"),
                detail: tr(
                    locale,
                    "Workspace modules can stay unpublished; external registry entries should declare a publisher.",
                    "Workspace-модули могут оставаться неопубликованными; внешние записи registry должны указывать publisher.",
                )
                .to_string(),
            }
        },
        if has_registry_publish_signal {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "ready",
                priority: "info",
                summary: tr(locale, "Present", "Есть"),
                detail: match (latest_release_version, latest_release_date) {
                    (Some(version), Some(date)) => {
                        format!(
                            "{} v{version} {} {date}.",
                            tr(locale, "Latest non-yanked release is", "Последний неотозванный релиз"),
                            tr(locale, "published at", "опубликован")
                        )
                    }
                    (Some(version), None) => {
                        format!(
                            "{} v{version}, {}.",
                            tr(locale, "Latest non-yanked release is", "Последний неотозванный релиз"),
                            tr(locale, "but publish date is missing", "но дата публикации отсутствует")
                        )
                    }
                    (None, _) => {
                        tr(
                            locale,
                            "Checksum is present even though no active version entry is visible.",
                            "Контрольная сумма есть, хотя активная запись версии не видна.",
                        )
                        .to_string()
                    }
                },
            }
        } else if has_yanked_only_versions {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "warn",
                priority: "info",
                summary: tr(locale, "Only yanked", "Только отозванные"),
                detail:
                    tr(
                        locale,
                        "Version history exists, but every visible release is yanked, so there is no active publish trail.",
                        "История версий существует, но все видимые релизы отозваны, поэтому активной publish-цепочки нет.",
                    )
                    .to_string(),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "info",
                priority: "info",
                summary: tr(locale, "Not published", "Не опубликован"),
                detail:
                    tr(
                        locale,
                        "No checksum or active version history is visible yet, which is expected for workspace-only modules.",
                        "Контрольная сумма и активная история версий пока не видны, что нормально для workspace-only модулей.",
                    )
                    .to_string(),
            }
        },
    ]
}

fn json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(value) => format!("{} items", value.len()),
        serde_json::Value::Object(value) => format!("{} keys", value.len()),
    }
}

fn parse_object_root(raw: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match parse_json_editor_value(raw, "object", Locale::en)? {
        Some(serde_json::Value::Object(object)) => Ok(object),
        Some(_) => Err("Expected a JSON object".to_string()),
        None => Ok(serde_json::Map::new()),
    }
}

fn parse_array_root(raw: &str) -> Result<Vec<serde_json::Value>, String> {
    match parse_json_editor_value(raw, "array", Locale::en)? {
        Some(serde_json::Value::Array(array)) => Ok(array),
        Some(_) => Err("Expected a JSON array".to_string()),
        None => Ok(Vec::new()),
    }
}

fn unique_object_key(
    object: &serde_json::Map<String, serde_json::Value>,
    preferred: &str,
) -> String {
    if !object.contains_key(preferred) {
        return preferred.to_string();
    }

    let mut index = 2;
    loop {
        let candidate = format!("{preferred}{index}");
        if !object.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn object_with_new_property(
    raw: &str,
    preferred_key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    let key = unique_object_key(&object, preferred_key);
    object.insert(key, value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_with_updated_property(
    raw: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.insert(key.to_string(), value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_without_property(raw: &str, key: &str) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.remove(key);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_with_renamed_property(raw: &str, old_key: &str, new_key: &str) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    let new_key = new_key.trim();
    if new_key.is_empty() {
        return Err("Property name must not be empty".to_string());
    }
    if old_key == new_key {
        return Ok(pretty_json_value(&serde_json::Value::Object(object)));
    }
    if object.contains_key(new_key) {
        return Err(format!("Property `{new_key}` already exists"));
    }
    let Some(value) = object.remove(old_key) else {
        return Err("Property key is out of bounds".to_string());
    };
    object.insert(new_key.to_string(), value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn array_with_appended_item(raw: &str, value: serde_json::Value) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    array.push(value);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_with_updated_item(
    raw: &str,
    index: usize,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    let Some(item) = array.get_mut(index) else {
        return Err("Array item is out of bounds".to_string());
    };
    *item = value;
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_without_item(raw: &str, index: usize) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    if index >= array.len() {
        return Err("Array item is out of bounds".to_string());
    }
    array.remove(index);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_item_moved(raw: &str, index: usize, delta: isize) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    if index >= array.len() {
        return Err("Array item is out of bounds".to_string());
    }
    let next_index = index as isize + delta;
    if next_index < 0 || next_index >= array.len() as isize {
        return Ok(pretty_json_value(&serde_json::Value::Array(array)));
    }
    array.swap(index, next_index as usize);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

#[derive(Clone, Debug)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

fn default_json_root(root_type: &str) -> serde_json::Value {
    match root_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        _ => serde_json::Value::Null,
    }
}

fn default_value_for_setting_type(value_type: &str) -> serde_json::Value {
    match value_type {
        "string" => serde_json::Value::String(String::new()),
        "integer" | "number" => serde_json::json!(0),
        "boolean" => serde_json::Value::Bool(false),
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        "json" | "any" => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    }
}

fn add_item_button_label(value_type: &str, locale: Locale) -> String {
    match value_type {
        "string" => tr(locale, "Add text", "Добавить текст").to_string(),
        "boolean" => tr(locale, "Add flag", "Добавить флаг").to_string(),
        "integer" | "number" => tr(locale, "Add number", "Добавить число").to_string(),
        "object" => tr(locale, "Add object", "Добавить объект").to_string(),
        "array" => tr(locale, "Add array", "Добавить массив").to_string(),
        "json" | "any" => tr(locale, "Add item", "Добавить элемент").to_string(),
        _ => format!(
            "{} {}",
            tr(locale, "Add", "Добавить"),
            humanize_token(value_type)
        ),
    }
}

fn parse_json_root(raw: &str, root_type: &str) -> Result<serde_json::Value, String> {
    Ok(parse_json_editor_value(raw, root_type, Locale::en)?
        .unwrap_or_else(|| default_json_root(root_type)))
}

fn value_at_path_mut<'a>(
    value: &'a mut serde_json::Value,
    path: &[JsonPathSegment],
) -> Option<&'a mut serde_json::Value> {
    let mut current = value;
    for segment in path {
        match segment {
            JsonPathSegment::Key(key) => current = current.as_object_mut()?.get_mut(key)?,
            JsonPathSegment::Index(index) => current = current.as_array_mut()?.get_mut(*index)?,
        }
    }
    Some(current)
}

fn with_updated_json_root(
    raw: &str,
    root_type: &str,
    updater: impl FnOnce(&mut serde_json::Value) -> Result<(), String>,
) -> Result<String, String> {
    let mut root = parse_json_root(raw, root_type)?;
    updater(&mut root)?;
    Ok(pretty_json_value(&root))
}

fn nested_value_updated(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    next_value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        *target = next_value;
        Ok(())
    })
}

fn nested_value_removed(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
) -> Result<String, String> {
    if path.is_empty() {
        return Ok(pretty_json_value(&default_json_root(root_type)));
    }

    let parent_path = &path[..path.len() - 1];
    let last_segment = path.last().expect("checked non-empty path");
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        match (parent, last_segment) {
            (serde_json::Value::Object(object), JsonPathSegment::Key(key)) => {
                object.remove(key);
                Ok(())
            }
            (serde_json::Value::Array(array), JsonPathSegment::Index(index)) => {
                if *index >= array.len() {
                    return Err("Array item is out of bounds".to_string());
                }
                array.remove(*index);
                Ok(())
            }
            _ => Err("JSON path does not match the current structure".to_string()),
        }
    })
}

fn nested_object_key_renamed(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    new_key: &str,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("JSON path is out of bounds".to_string());
    }
    let new_key = new_key.trim();
    if new_key.is_empty() {
        return Err("Property name must not be empty".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let JsonPathSegment::Key(old_key) = path.last().expect("checked non-empty path") else {
        return Err("JSON path does not point to an object property".to_string());
    };
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(object) = parent.as_object_mut() else {
            return Err("Expected a JSON object".to_string());
        };
        if old_key == new_key {
            return Ok(());
        }
        if object.contains_key(new_key) {
            return Err(format!("Property `{new_key}` already exists"));
        }
        let Some(value) = object.remove(old_key) else {
            return Err("JSON path is out of bounds".to_string());
        };
        object.insert(new_key.to_string(), value);
        Ok(())
    })
}

fn nested_array_item_moved(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    delta: isize,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("JSON path is out of bounds".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let JsonPathSegment::Index(index) = path.last().expect("checked non-empty path") else {
        return Err("JSON path does not point to an array item".to_string());
    };
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(array) = parent.as_array_mut() else {
            return Err("Expected a JSON array".to_string());
        };
        if *index >= array.len() {
            return Err("Array item is out of bounds".to_string());
        }
        let next_index = *index as isize + delta;
        if next_index < 0 || next_index >= array.len() as isize {
            return Ok(());
        }
        array.swap(*index, next_index as usize);
        Ok(())
    })
}

fn nested_object_child_added(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    preferred_key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(object) = target.as_object_mut() else {
            return Err("Expected a JSON object".to_string());
        };
        let key = unique_object_key(object, preferred_key);
        object.insert(key, value);
        Ok(())
    })
}

fn nested_array_child_added(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(array) = target.as_array_mut() else {
            return Err("Expected a JSON array".to_string());
        };
        array.push(value);
        Ok(())
    })
}

fn nested_object_contains_key(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    key: &str,
) -> bool {
    let Ok(mut root) = parse_json_root(raw, root_type) else {
        return false;
    };

    value_at_path_mut(&mut root, path)
        .and_then(|target| target.as_object().map(|object| object.contains_key(key)))
        .unwrap_or(false)
}

fn render_nested_json_children(
    root_type: String,
    root_value: Signal<String>,
    path: Vec<JsonPathSegment>,
    current: serde_json::Value,
    current_shape: Option<serde_json::Value>,
    locale: Locale,
    disabled: Signal<bool>,
    on_input: Callback<String>,
) -> AnyView {
    match current {
        serde_json::Value::Object(object) => {
            let declared_properties = setting_shape_properties(current_shape.as_ref());
            let schema_locks_keys = !declared_properties.is_empty();
            view! {
            <div class="space-y-3">
                <div class="flex flex-wrap gap-2">
                    {if declared_properties.is_empty() {
                        view! {
                            <>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newText", serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add text", "Добавить текст")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newFlag", serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add flag", "Добавить флаг")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newNumber", serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add number", "Добавить число")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newObject", serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add object", "Добавить объект")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newArray", serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add array", "Добавить массив")}</button>
                            </>
                        }.into_any()
                    } else {
                        declared_properties.clone().into_iter().map(|(property_key, property_shape)| {
                            let button_label = format!("Add {}", humanize_setting_key(&property_key));
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled={
                                        let root_type = root_type.clone();
                                        let root_value = root_value;
                                        let path = path.clone();
                                        let property_key = property_key.clone();
                                        move || {
                                            disabled.get()
                                                || nested_object_contains_key(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &path,
                                                    &property_key,
                                                )
                                        }
                                    }
                                    on:click={
                                        let root_type = root_type.clone();
                                        let root_value = root_value;
                                        let path = path.clone();
                                        let property_key = property_key.clone();
                                        let property_shape = property_shape.clone();
                                        move |_| {
                                            if let Ok(next) = nested_object_child_added(
                                                &root_value.get(),
                                                &root_type,
                                                &path,
                                                &property_key,
                                                default_value_for_schema_shape(Some(&property_shape)),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }
                        }).collect_view().into_any()
                    }}
                </div>
                {object.into_iter().map(|(key, item_value)| {
                    let kind = json_value_kind(&item_value).to_string();
                    let preview = json_value_preview(&item_value);
                    let property_shape = setting_shape_property(current_shape.as_ref(), &key);
                    let mut item_path = path.clone();
                    item_path.push(JsonPathSegment::Key(key.clone()));
                    match item_value.clone() {
                        scalar_value @ (serde_json::Value::String(_) | serde_json::Value::Bool(_) | serde_json::Value::Number(_)) => {
                            let item_path_for_input = item_path.clone();
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_rename = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |event| {
                                                    if let Ok(next) = nested_object_key_renamed(&root_value.get(), &root_type, &item_path_for_rename, &event_target_value(&event)) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            } />
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>{tr(locale, "Remove", "Удалить")}</button>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        property_shape.clone(),
                                        locale,
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |next_value| {
                                                if let Ok(next) = nested_value_updated(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &item_path_for_input,
                                                    next_value,
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }),
                                    )}
                                </div>
                            }.into_any()
                        }
                        _ => {
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_rename = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |event| {
                                                    if let Ok(next) = nested_object_key_renamed(&root_value.get(), &root_type, &item_path_for_rename, &event_target_value(&event)) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            } />
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>{tr(locale, "Remove", "Удалить")}</button>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(root_type.clone(), root_value, item_path.clone(), item_value, property_shape.clone(), locale, disabled, on_input)}
                                </div>
                            }.into_any()
                        }
                    }
                }).collect_view()}
            </div>
        }.into_any()
        }
        serde_json::Value::Array(items) => {
            let item_shape = setting_shape_items(current_shape.as_ref());
            view! {
            <div class="space-y-3">
                <div class="flex flex-wrap gap-2">
                    {if let Some(item_shape) = item_shape.clone() {
                        let button_label = schema_action_label(Some(&item_shape), locale);
                        view! {
                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                let root_type = root_type.clone();
                                let root_value = root_value;
                                let path = path.clone();
                                let item_shape = item_shape.clone();
                                move |_| {
                                    if let Ok(next) = nested_array_child_added(
                                        &root_value.get(),
                                        &root_type,
                                        &path,
                                        default_value_for_schema_shape(Some(&item_shape)),
                                    ) {
                                        on_input.run(next);
                                    }
                                }
                            }>{button_label}</button>
                        }.into_any()
                    } else {
                        view! {
                            <>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add text", "Добавить текст")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add flag", "Добавить флаг")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add number", "Добавить число")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add object", "Добавить объект")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add array", "Добавить массив")}</button>
                            </>
                        }.into_any()
                    }}
                </div>
                {items.into_iter().enumerate().map(|(index, item_value)| {
                    let kind = json_value_kind(&item_value).to_string();
                    let preview = json_value_preview(&item_value);
                    let mut item_path = path.clone();
                    item_path.push(JsonPathSegment::Index(index));
                    match item_value.clone() {
                        scalar_value @ (serde_json::Value::String(_)
                        | serde_json::Value::Bool(_)
                        | serde_json::Value::Number(_)) => {
                            let item_path_for_input = item_path.clone();
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_move_up = item_path.clone();
                            let item_path_for_move_down = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Up", "Вверх")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Down", "Вниз")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Remove", "Удалить")}</button>
                                        </div>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        item_shape.clone(),
                                        locale,
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |next_value| {
                                                if let Ok(next) = nested_value_updated(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &item_path_for_input,
                                                    next_value,
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }),
                                    )}
                                </div>
                            }.into_any()
                        }
                        _ => {
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_move_up = item_path.clone();
                            let item_path_for_move_down = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Up", "Вверх")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Down", "Вниз")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Remove", "Удалить")}</button>
                                        </div>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(root_type.clone(), root_value, item_path.clone(), item_value, item_shape.clone(), locale, disabled, on_input)}
                                </div>
                            }.into_any()
                        }
                    }
                }).collect_view()}
            </div>
        }.into_any()
        }
        _ => ().into_any(),
    }
}

#[component]
fn StructuredObjectEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    object_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let object_entries = Signal::derive(move || parse_object_root(&value.get()));
    let declared_properties = setting_shape_properties(object_shape.as_ref());
    let object_shape_for_items = StoredValue::new(object_shape.clone());
    let schema_locks_keys = !declared_properties.is_empty();

    view! {
        <Show when=move || object_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        {tr(locale, "Structured object editor", "Структурный редактор объекта")}
                    </p>
                    <div class="flex flex-wrap gap-2">
                        {if declared_properties.is_empty() {
                            view! {
                                <>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newText",
                                                    serde_json::Value::String(String::new()),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add text", "Добавить текст")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newFlag",
                                                    serde_json::Value::Bool(false),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add flag", "Добавить флаг")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newNumber",
                                                    serde_json::json!(0),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add number", "Добавить число")}
                                    </button>
                                </>
                            }.into_any()
                        } else {
                            declared_properties
                                .clone()
                                .into_iter()
                                .map(|(property_key, property_shape)| {
                                    let button_label = format!(
                                        "{} {}",
                                        tr(locale, "Add", "Добавить"),
                                        humanize_setting_key(&property_key)
                                    );
                                    view! {
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                            disabled={
                                                let value = value;
                                                let property_key = property_key.clone();
                                                move || {
                                                    disabled.get()
                                                        || parse_object_root(&value.get())
                                                            .map(|object| object.contains_key(&property_key))
                                                            .unwrap_or(false)
                                                }
                                            }
                                            on:click={
                                                let value = value;
                                                let property_key = property_key.clone();
                                                let property_shape = property_shape.clone();
                                                move |_| {
                                                    if let Ok(next) = object_with_updated_property(
                                                        &value.get(),
                                                        &property_key,
                                                        default_value_for_schema_shape(Some(&property_shape)),
                                                    ) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }
                                        >
                                            {button_label}
                                        </button>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </div>
                <div class="mt-3 space-y-3">
                    {move || {
                        let object_shape_for_items = object_shape_for_items.get_value();
                        match object_entries.get() {
                        Ok(object) if object.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                {tr(locale, "Object is empty. Use the quick actions to add top-level properties.", "Объект пуст. Используйте быстрые действия, чтобы добавить поля верхнего уровня.")}
                            </p>
                        }.into_any(),
                        Ok(object) => object
                            .into_iter()
                            .map(|(key, item_value)| {
                                let kind = json_value_kind(&item_value).to_string();
                                let preview = json_value_preview(&item_value);
                                let property_shape = setting_shape_property(object_shape_for_items.as_ref(), &key);
                                let key_for_remove = key.clone();
                                let key_for_rename = key.clone();
                                let mut item_path = Vec::new();
                                item_path.push(JsonPathSegment::Key(key.clone()));
                                view! {
                                    <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                    let value = value;
                                                    move |event| {
                                                        if let Ok(next) = object_with_renamed_property(&value.get(), &key_for_rename, &event_target_value(&event)) {
                                                            on_input.run(next);
                                                        }
                                                    }
                                                } />
                                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                    {kind.clone()}
                                                </span>
                                            </div>
                                            <button
                                                type="button"
                                                class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                disabled=move || disabled.get()
                                                on:click={
                                                    let value = value;
                                                    move |_| {
                                                        if let Ok(next) = object_without_property(&value.get(), &key_for_remove) {
                                                            on_input.run(next);
                                                        }
                                                    }
                                                }
                                            >
                                                {tr(locale, "Remove", "Удалить")}
                                            </button>
                                        </div>
                                        {match item_value {
                                            scalar_value @ (serde_json::Value::String(_)
                                            | serde_json::Value::Bool(_)
                                            | serde_json::Value::Number(_)) => {
                                                let key_for_input = key.clone();
                                                view! {
                                                    {render_scalar_value_editor(
                                                        scalar_value,
                                                        property_shape.clone(),
                                                        locale,
                                                        disabled,
                                                        Callback::new({
                                                            let value = value;
                                                            move |next_value| {
                                                                if let Ok(next) = object_with_updated_property(
                                                                    &value.get(),
                                                                    &key_for_input,
                                                                    next_value,
                                                                ) {
                                                                    on_input.run(next);
                                                                }
                                                            }
                                                        }),
                                                    )}
                                                }.into_any()
                                            }
                                            nested_value => {
                                                let nested_path = item_path.clone();
                                                let nested_shape = property_shape.clone();
                                                view! {
                                                    <>
                                                        <p class="text-sm text-muted-foreground">
                                                            {format!(
                                                                "{} {}: {}.",
                                                                tr(locale, "Nested", "Вложенный"),
                                                                kind,
                                                                preview
                                                            )}
                                                        </p>
                                                        {render_nested_json_children(
                                                            "object".to_string(),
                                                            value,
                                                            nested_path,
                                                            nested_value,
                                                            nested_shape,
                                                            locale,
                                                            disabled,
                                                            on_input,
                                                        )}
                                                    </>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any(),
                        Err(_) => ().into_any(),
                    }}}
                </div>
            </div>
        </Show>
    }
}

#[component]
fn StructuredArrayEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    array_item_type: Option<String>,
    array_item_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let array_entries = Signal::derive(move || parse_array_root(&value.get()));
    let array_item_shape_for_items = StoredValue::new(array_item_shape.clone());

    view! {
        <Show when=move || array_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        {tr(locale, "Structured array editor", "Структурный редактор массива")}
                    </p>
                    <div class="flex flex-wrap gap-2">
                        {if let Some(item_shape) = array_item_shape.clone() {
                            let button_label = schema_action_label(Some(&item_shape), locale);
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
                                        let value = value;
                                        let item_shape = item_shape.clone();
                                        move |_| {
                                            if let Ok(next) = array_with_appended_item(
                                                &value.get(),
                                                default_value_for_schema_shape(Some(&item_shape)),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }.into_any()
                        } else if let Some(item_type) = array_item_type
                            .clone()
                            .map(|value| value.trim().to_string())
                            .filter(|value| !value.is_empty())
                        {
                            let button_label = add_item_button_label(&item_type, locale);
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
                                        let value = value;
                                        let item_type = item_type.clone();
                                        move |_| {
                                            if let Ok(next) = array_with_appended_item(
                                                &value.get(),
                                                default_value_for_setting_type(&item_type),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }.into_any()
                        } else {
                            view! {
                                <>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = array_with_appended_item(
                                                    &value.get(),
                                                    serde_json::Value::String(String::new()),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add text", "Добавить текст")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = array_with_appended_item(
                                                    &value.get(),
                                                    serde_json::Value::Bool(false),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add flag", "Добавить флаг")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) =
                                                    array_with_appended_item(&value.get(), serde_json::json!(0))
                                                {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add number", "Добавить число")}
                                    </button>
                                </>
                            }.into_any()
                        }}
                    </div>
                </div>
                <div class="mt-3 space-y-3">
                    {move || {
                        let array_item_shape_for_items = array_item_shape_for_items.get_value();
                        match array_entries.get() {
                        Ok(items) if items.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                {tr(locale, "Array is empty. Use the quick actions to add top-level items.", "Массив пуст. Используйте быстрые действия, чтобы добавить элементы верхнего уровня.")}
                            </p>
                        }.into_any(),
                        Ok(items) => items
                            .into_iter()
                            .enumerate()
                            .map(|(index, item_value)| {
                                let kind = json_value_kind(&item_value).to_string();
                                let preview = json_value_preview(&item_value);
                                let mut item_path = Vec::new();
                                item_path.push(JsonPathSegment::Index(index));
                                view! {
                                    <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
                                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                    {kind.clone()}
                                                </span>
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, -1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Up", "Вверх")}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, 1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Down", "Вниз")}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_without_item(&value.get(), index) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Remove", "Удалить")}
                                                </button>
                                            </div>
                                        </div>
                                        {match item_value {
                                            scalar_value @ (serde_json::Value::String(_)
                                            | serde_json::Value::Bool(_)
                                            | serde_json::Value::Number(_)) => {
                                                view! {
                                                    {render_scalar_value_editor(
                                                        scalar_value,
                                                        array_item_shape_for_items.clone(),
                                                        locale,
                                                        disabled,
                                                        Callback::new({
                                                            let value = value;
                                                            move |next_value| {
                                                                if let Ok(next) = array_with_updated_item(
                                                                    &value.get(),
                                                                    index,
                                                                    next_value,
                                                                ) {
                                                                    on_input.run(next);
                                                                }
                                                            }
                                                        }),
                                                    )}
                                                }.into_any()
                                            }
                                            nested_value => {
                                                let nested_path = item_path.clone();
                                                let nested_shape = array_item_shape_for_items.clone();
                                                view! {
                                                    <>
                                                        <p class="text-sm text-muted-foreground">
                                                            {format!(
                                                                "{} {}: {}.",
                                                                tr(locale, "Nested", "Вложенный"),
                                                                kind,
                                                                preview
                                                            )}
                                                        </p>
                                                    {render_nested_json_children(
                                                        "array".to_string(),
                                                        value,
                                                        nested_path,
                                                        nested_value,
                                                        nested_shape.or_else(|| array_item_shape_for_items.clone()),
                                                        locale,
                                                        disabled,
                                                        on_input,
                                                    )}
                                                    </>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any(),
                        Err(_) => ().into_any(),
                    }}}
                </div>
            </div>
        </Show>
    }
}

#[component]
fn ComplexSettingEditor(
    field_type: String,
    placeholder: &'static str,
    array_item_type: Option<String>,
    schema_shape: Option<serde_json::Value>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let status = Signal::derive({
        let value = value;
        let field_type = field_type.clone();
        move || json_editor_summary(&field_type, &value.get(), locale)
    });
    let nested_root = Signal::derive({
        let value = value;
        let field_type = field_type.clone();
        move || parse_json_root(&value.get(), &field_type).ok()
    });

    let show_add_button = matches!(field_type.as_str(), "object" | "array");
    let add_button_label = if field_type == "object" {
        tr(locale, "Add property", "Добавить поле")
    } else {
        tr(locale, "Add item", "Добавить элемент")
    };

    view! {
        <div class="space-y-3">
            <div class="flex flex-wrap items-center gap-2 text-xs">
                <span class=move || {
                    if status.get().0 {
                        "inline-flex items-center rounded-full border border-border px-2 py-0.5 font-medium text-muted-foreground"
                    } else {
                        "inline-flex items-center rounded-full border border-destructive/40 bg-destructive/10 px-2 py-0.5 font-medium text-destructive"
                    }
                }>
                    {move || status.get().1}
                </span>
                <Show when=move || !status.get().2.is_empty()>
                    <div class="flex flex-wrap gap-1">
                        {move || status.get().2.into_iter().map(|item| {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground">
                                    {item}
                                </span>
                            }
                        }).collect_view()}
                    </div>
                </Show>
            </div>
            {if field_type == "object" {
                view! { <StructuredObjectEditor value=value disabled=disabled object_shape=schema_shape.clone() on_input=on_input /> }.into_any()
            } else if field_type == "array" {
                view! {
                    <StructuredArrayEditor
                        value=value
                        disabled=disabled
                        array_item_type=array_item_type.clone()
                        array_item_shape=setting_shape_items(schema_shape.as_ref())
                        on_input=on_input
                    />
                }.into_any()
            } else {
                ().into_any()
            }}
            {if matches!(field_type.as_str(), "json" | "any") {
                {
                    let field_type_for_nested = field_type.clone();
                    move || {
                    nested_root
                        .get()
                        .filter(|value| matches!(value, serde_json::Value::Object(_) | serde_json::Value::Array(_)))
                        .map(|root| {
                            view! {
                                <div class="space-y-2 rounded-lg border border-border/60 bg-background/60 p-3">
                                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                                        {tr(locale, "Nested editor", "Вложенный редактор")}
                                    </p>
                                    {render_nested_json_children(
                                        field_type_for_nested.clone(),
                                        value,
                                        Vec::new(),
                                        root,
                                        schema_shape.clone(),
                                        locale,
                                        disabled,
                                        on_input,
                                    )}
                                </div>
                            }
                        })
                    }
                }.into_any()
            } else {
                ().into_any()
            }}
            <div class="flex flex-wrap items-center gap-2">
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                    disabled=move || disabled.get()
                    on:click={
                        let value = value;
                        let field_type = field_type.clone();
                        move |_| {
                            match parse_json_editor_value(&value.get(), &field_type, locale) {
                                Ok(Some(parsed)) => on_input.run(pretty_json_value(&parsed)),
                                Ok(None) => on_input.run(reset_json_editor_value(&field_type)),
                                Err(_) => {}
                            }
                        }
                    }
                >
                    {tr(locale, "Format JSON", "Форматировать JSON")}
                </button>
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                    disabled=move || disabled.get()
                    on:click={
                        let field_type = field_type.clone();
                        move |_| on_input.run(reset_json_editor_value(&field_type))
                    }
                >
                    {tr(locale, "Reset", "Сбросить")}
                </button>
                {if show_add_button {
                    view! {
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || disabled.get()
                            on:click={
                                let value = value;
                                let field_type = field_type.clone();
                                move |_| {
                                    let next = match field_type.as_str() {
                                        "object" => append_object_property(&value.get()),
                                        "array" => append_array_item(&value.get()),
                                        _ => Ok(value.get()),
                                    };
                                    if let Ok(next) = next {
                                        on_input.run(next);
                                    }
                                }
                            }
                        >
                            {add_button_label}
                        </button>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
            <textarea
                class="min-h-32 w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                prop:value=move || value.get()
                prop:placeholder=placeholder
                disabled=move || disabled.get()
                on:input=move |event| on_input.run(event_target_value(&event))
            ></textarea>
        </div>
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
    let default_actor = module
        .as_ref()
        .and_then(|module| {
            module
                .registry_lifecycle
                .as_ref()
                .and_then(|lifecycle| lifecycle.owner_binding.as_ref())
                .map(|owner| owner.owner_actor.clone())
                .or_else(|| {
                    module
                        .registry_lifecycle
                        .as_ref()
                        .and_then(|lifecycle| lifecycle.latest_request.as_ref())
                        .map(|request| request.requested_by.clone())
                })
        })
        .unwrap_or_else(|| "governance:platform".to_string());
    let default_publisher = module
        .as_ref()
        .and_then(|module| {
            module
                .registry_lifecycle
                .as_ref()
                .and_then(|lifecycle| lifecycle.latest_request.as_ref())
                .and_then(|request| request.publisher_identity.clone())
                .or_else(|| module.publisher.clone())
        })
        .unwrap_or_default();
    let default_new_owner_actor = module
        .as_ref()
        .and_then(|module| {
            let lifecycle = module.registry_lifecycle.as_ref()?;
            let requested = lifecycle
                .latest_request
                .as_ref()
                .and_then(|request| request.publisher_identity.as_ref())?;
            if lifecycle
                .owner_binding
                .as_ref()
                .is_none_or(|owner| owner.owner_actor != *requested)
            {
                Some(requested.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let (governance_actor, set_governance_actor) = signal(default_actor);
    let (governance_publisher, set_governance_publisher) = signal(default_publisher);
    let (governance_reason, set_governance_reason) = signal(String::new());
    let (governance_new_owner_actor, set_governance_new_owner_actor) =
        signal(default_new_owner_actor);
    let (governance_dry_run, set_governance_dry_run) = signal(false);
    let (governance_submitting, set_governance_submitting) = signal(false);
    let (governance_feedback, set_governance_feedback) = signal(None::<String>);
    let (governance_error, set_governance_error) = signal(None::<String>);
    let (governance_result, set_governance_result) = signal(None::<RegistryMutationResult>);
    let (governance_confirmation_action, set_governance_confirmation_action) =
        signal(None::<String>);

    view! {
        <div class="rounded-xl border border-primary/20 bg-primary/5 p-6 shadow-sm">
            <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">
                        {tr(locale, "Module detail", "Детали модуля")}
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
                                    "— метаданные из внутреннего marketplace-каталога.",
                                )
                            ),
                            None if loading.get() => format!(
                                "{} {}",
                                tr(locale, "Loading", "Загрузка"),
                                selected_slug
                            ),
                            None => format!(
                                "{} {}.",
                                tr(locale, "No catalog entry resolved for", "Не удалось найти запись каталога для"),
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
                    {tr(locale, "Close", "Закрыть")}
                </button>
            </div>

            <Show
                when=move || detail.is_some()
                fallback=move || view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        {tr(
                            locale,
                            "The selected module is not available in the current catalog snapshot.",
                            "Выбранный модуль недоступен в текущем снимке каталога.",
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
                            lifecycle_detail_lines(latest_registry_request.as_ref(), locale);
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
                        let can_validate = latest_registry_request.as_ref().is_some_and(|request| {
                            status_eq(&request.status, "artifact_uploaded")
                                || status_eq(&request.status, "submitted")
                        });
                        let can_approve = latest_registry_request
                            .as_ref()
                            .is_some_and(|request| status_eq(&request.status, "approved"));
                        let can_reject = latest_registry_request.as_ref().is_some_and(|request| {
                            !status_eq(&request.status, "rejected")
                                && !status_eq(&request.status, "published")
                        });
                        let can_transfer_owner = latest_registry_request.as_ref().is_some_and(|request| {
                            request.publisher_identity.as_ref().is_some_and(|publisher| {
                                registry_owner_binding
                                    .as_ref()
                                    .is_none_or(|owner| owner.owner_actor != *publisher)
                            })
                        }) || registry_owner_binding.is_some();
                        let can_yank = latest_registry_release.as_ref().is_some_and(|release| {
                            status_eq(&release.status, "published")
                                || status_eq(&release.status, "active")
                        });
                        let recent_governance_events = module
                            .registry_lifecycle
                            .as_ref()
                            .map(|lifecycle| lifecycle.recent_events.clone())
                            .unwrap_or_default();
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
                        let show_validation_summary = has_validation_warnings
                            || has_validation_errors
                            || validation_rejection_reason.is_some()
                            || validation_outcome_summary.is_some()
                            || review_ready
                            || latest_validation_event_summary.is_some();
                        let show_follow_up_gates = !follow_up_gates.is_empty();
                        let show_validation_stages = !validation_stages.is_empty();
                        let governance_hint = registry_governance_hint(&module, locale);
                        let checksum = short_checksum(module.checksum_sha256.as_deref());
                        let request_id = latest_registry_request.as_ref().map(|request| request.id.clone());
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
                        let refresh_detail_after_reject = on_refresh_detail.clone();
                        let refresh_detail_after_transfer = on_refresh_detail.clone();
                        let refresh_detail_after_yank = on_refresh_detail.clone();
                        let on_validate_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "Нет доступного publish-запроса.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let actor = governance_actor.get_untracked().trim().to_string();
                                if actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Actor is required.", "Нужно указать actor.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let dry_run = governance_dry_run.get_untracked();
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match api::validate_registry_publish_request(
                                        request_id,
                                        actor,
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
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "Нет доступного publish-запроса.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let actor = governance_actor.get_untracked().trim().to_string();
                                if actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Actor is required.", "Нужно указать actor.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let dry_run = governance_dry_run.get_untracked();
                                let publisher = governance_publisher
                                    .get_untracked()
                                    .trim()
                                    .to_string();
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match api::approve_registry_publish_request(
                                        request_id,
                                        actor,
                                        (!publisher.is_empty()).then_some(publisher),
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
                        let on_reject_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            let module_slug_for_actions = module_slug_for_actions.clone();
                            Callback::new(move |_| {
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "Нет доступного publish-запроса.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let actor = governance_actor.get_untracked().trim().to_string();
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                if actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Actor is required.", "Нужно указать actor.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if reason.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "Нужно указать причину.")
                                            .to_string(),
                                    ));
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
                                    match api::reject_registry_publish_request(
                                        request_id,
                                        actor,
                                        reason,
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
                                let actor = governance_actor.get_untracked().trim().to_string();
                                let new_owner_actor =
                                    governance_new_owner_actor.get_untracked().trim().to_string();
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                if actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Actor is required.", "Нужно указать actor.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if new_owner_actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(
                                            locale,
                                            "New owner actor is required.",
                                            "Нужно указать нового владельца."
                                        )
                                        .to_string(),
                                    ));
                                    return;
                                }
                                if reason.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "Нужно указать причину.")
                                            .to_string(),
                                    ));
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
                                            Some(&new_owner_actor),
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
                                    match api::transfer_registry_owner(
                                        module_slug_for_actions.clone(),
                                        actor,
                                        new_owner_actor,
                                        reason,
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
                                let actor = governance_actor.get_untracked().trim().to_string();
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                if actor.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Actor is required.", "Нужно указать actor.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if reason.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "Нужно указать причину.")
                                            .to_string(),
                                    ));
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
                                    match api::yank_registry_release(
                                        module_slug_for_actions.clone(),
                                        release_version.clone(),
                                        actor,
                                        reason,
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
                                                tr(locale, "Compatible", "Совместим")
                                            } else {
                                                tr(locale, "Compatibility risk", "Риск совместимости")
                                            }}
                                        </span>
                                        {module.signature_present.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {tr(locale, "Signed", "Подписан")}
                                            </span>
                                        })}
                                        {module.installed.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {format!(
                                                    "{}{}",
                                                    tr(locale, "Installed", "Установлен"),
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
                                                {tr(locale, "Update available", "Доступно обновление")}
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
                                            tr(locale, "Primary for this admin", "Primary для этой admin-поверхности")
                                        } else if showcase_here {
                                            tr(locale, "Showcase for this admin", "Showcase для этой admin-поверхности")
                                        } else {
                                            tr(locale, "No dedicated UI for this admin", "Для этой admin-поверхности нет выделенного UI")
                                        }}
                                    </span>
                                </div>

                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Package metadata", "Метаданные пакета")}
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
                                                <dt class="text-muted-foreground">{tr(locale, "Publisher", "Издатель")}</dt>
                                                <dd class="text-right">{module.publisher.clone().unwrap_or_else(|| tr(locale, "Workspace / unknown", "Workspace / неизвестно").to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "RusTok range", "Диапазон RusTok")}</dt>
                                                <dd class="text-right">
                                                    {format!(
                                                        "{}{}",
                                                        module
                                                            .rustok_min_version
                                                            .as_ref()
                                                            .map(|value| format!(">= {}", value))
                                                            .unwrap_or_else(|| tr(locale, "no min", "без min").to_string()),
                                                        module
                                                            .rustok_max_version
                                                            .as_ref()
                                                            .map(|value| format!(", <= {}", value))
                                                            .unwrap_or_else(|| format!(", {}", tr(locale, "no max", "без max")))
                                                    )}
                                                </dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Checksum", "Контрольная сумма")}</dt>
                                                <dd class="font-mono text-right">{checksum.unwrap_or_else(|| tr(locale, "Not published", "Не опубликован").to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Latest published", "Последняя публикация")}</dt>
                                                <dd class="text-right">
                                                    {latest_release
                                                        .as_ref()
                                                        .map(|version| format!(
                                                            "v{}{}",
                                                            version.version,
                                                            version
                                                                .published_at
                                                                .as_ref()
                                                                .map(|value| format!(" · {}", value))
                                                                .unwrap_or_default()
                                                        ))
                                                        .unwrap_or_else(|| tr(locale, "No active release", "Нет активного релиза").to_string())}
                                                </dd>
                                            </div>
                                        </dl>
                                    </div>

                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Surface policy", "Политика поверхностей")}
                                        </p>
                                        <div class="mt-3 space-y-3">
                                            <div class="flex flex-wrap gap-2">
                                                {if module.recommended_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            {tr(locale, "No primary admin surface declared.", "Primary admin-поверхность не объявлена.")}
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
                                                            {tr(locale, "No showcase admin surface declared.", "Showcase admin-поверхность не объявлена.")}
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
                                                    tr(locale, "No module dependencies declared.", "Зависимости модуля не объявлены.").to_string()
                                                } else {
                                                    format!("{}: {}", tr(locale, "Depends on", "Зависит от"), module.dependencies.join(", "))
                                                }}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Publish lifecycle", "Жизненный цикл публикации")}
                                        </p>
                                        {latest_registry_request.as_ref().map(|request| {
                                            view! {
                                                <span class=registry_request_status_badge_classes(&request.status)>
                                                    {format!("{}: {}", tr(locale, "Request", "Запрос"), humanize_token(&request.status))}
                                                </span>
                                            }
                                        })}
                                        {latest_registry_release.as_ref().map(|release| {
                                            view! {
                                                <span class=registry_request_status_badge_classes(&release.status)>
                                                    {format!("{}: {}", tr(locale, "Release", "Релиз"), humanize_token(&release.status))}
                                                </span>
                                            }
                                        })}
                                        {if latest_registry_request.is_none() && latest_registry_release.is_none() {
                                            view! {
                                                <span class=registry_request_status_badge_classes("info")>
                                                    {tr(locale, "No V2 activity yet", "Активности V2 пока нет")}
                                                </span>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                    <dl class="mt-3 space-y-2">
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner binding", "Связка владельца")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.owner_actor.clone())
                                                    .unwrap_or_else(|| tr(locale, "No persisted owner binding", "Нет сохранённой связки владельца").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner bound by", "Кем привязан владелец")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.bound_by.clone())
                                                    .unwrap_or_else(|| tr(locale, "No owner transfer history", "Истории привязки владельца нет").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner updated", "Владелец обновлён")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.updated_at.clone())
                                                    .unwrap_or_else(|| tr(locale, "No owner activity", "Активности по владельцу нет").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Latest request", "Последний запрос")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| format!("{} · {}", request.id, humanize_token(&request.status)))
                                                    .unwrap_or_else(|| tr(locale, "No publish request recorded", "Запросов на публикацию пока нет").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request actor", "Актор запроса")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| request.requested_by.clone())
                                                    .unwrap_or_else(|| tr(locale, "Unknown", "Неизвестно").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request publisher", "Издатель запроса")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .and_then(|request| request.publisher_identity.clone())
                                                    .unwrap_or_else(|| tr(locale, "Not persisted", "Не сохранён").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request updated", "Запрос обновлён")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| request.updated_at.clone())
                                                    .unwrap_or_else(|| tr(locale, "No request activity", "Активности по запросу нет").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Latest release state", "Состояние последнего релиза")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_release
                                                    .as_ref()
                                                    .map(|release: &RegistryReleaseLifecycle| format!(
                                                        "v{} · {}{}",
                                                        release.version,
                                                        humanize_token(&release.status),
                                                        if status_eq(&release.status, "yanked") {
                                                            release
                                                                .yanked_at
                                                                .as_ref()
                                                                .map(|value| format!(" · {}", value))
                                                                .unwrap_or_default()
                                                        } else {
                                                            format!(" · {}", release.published_at)
                                                        }
                                                    ))
                                                    .unwrap_or_else(|| tr(locale, "No persisted release state", "Сохранённого состояния релиза нет").to_string())}
                                            </dd>
                                        </div>
                                    </dl>
                                    <p class="mt-3 text-xs text-muted-foreground">{governance_hint}</p>
                                    <Show when=move || show_validation_summary>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Validation summary", "Сводка валидации")}
                                            </p>
                                            <div class="flex flex-wrap gap-2 text-xs">
                                                {validation_outcome_summary.as_ref().map(|outcome| {
                                                    view! {
                                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Outcome", "Итог"), outcome)}
                                                        </span>
                                                    }
                                                })}
                                                <Show when=move || review_ready>
                                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                        {tr(locale, "Ready for review", "Готов к review")}
                                                    </span>
                                                </Show>
                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                    {format!("{}: {}", tr(locale, "Warnings", "Предупреждения"), validation_warning_items.len())}
                                                </span>
                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                    {format!("{}: {}", tr(locale, "Errors", "Ошибки"), validation_error_items.len())}
                                                </span>
                                                {latest_validation_event_summary.as_ref().map(|(title, _, created_at, _)| {
                                                    view! {
                                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                            {format!("{}: {} · {}", tr(locale, "Last event", "Последнее событие"), title, created_at)}
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
                                                            {format!("{}: {} · {}", tr(locale, "Actor", "Actor"), actor, created_at)}
                                                        </p>
                                                    </div>
                                                }
                                            })}
                                            <Show when=move || has_validation_warnings>
                                                <div class="rounded-lg border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                                                    <p class="font-medium">{tr(locale, "Warnings", "Предупреждения")}</p>
                                                    <div class="mt-2 space-y-1">
                                                        {validation_warning_items_for_show.get_value().into_iter().map(|warning| {
                                                            view! { <p>{warning}</p> }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </Show>
                                            <Show when=move || has_validation_errors>
                                                <div class="rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-700">
                                                    <p class="font-medium">{tr(locale, "Errors", "Ошибки")}</p>
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
                                                        <p class="font-medium">{tr(locale, "Rejection reason", "Причина отклонения")}</p>
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
                                                            {format!("{}: {}", tr(locale, "Attempt", "Попытка"), stage.attempt_number)}
                                                        </p>
                                                        {stage.started_at.as_ref().map(|started_at| {
                                                            view! {
                                                                <p class="mt-1 text-[11px] text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Started", "Начато"), started_at)}
                                                                </p>
                                                            }
                                                        })}
                                                        {stage.finished_at.as_ref().map(|finished_at| {
                                                            view! {
                                                                <p class="mt-1 text-[11px] text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Finished", "Завершено"), finished_at)}
                                                                </p>
                                                            }
                                                        })}
                                                        <p class="mt-1 text-[11px] text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Updated", "РћР±РЅРѕРІР»РµРЅРѕ"), stage.updated_at)}
                                                        </p>
                                                        <Show when=move || has_stage_history>
                                                            <div class="mt-2 space-y-2 border-t border-border/70 pt-2">
                                                                <p class="text-[11px] uppercase tracking-wide text-muted-foreground">
                                                                    {tr(locale, "Recent stage history", "Недавняя история этапа")}
                                                                </p>
                                                                {stage_history_for_show.get_value().into_iter().map(|event| {
                                                                    let title = governance_event_title(&event.event_type, locale);
                                                                    let summary = governance_event_summary(&event, locale);
                                                                    view! {
                                                                        <div class="rounded border border-border/70 bg-background/80 px-2 py-2 text-[11px] text-muted-foreground">
                                                                            <p class="font-medium text-card-foreground">{title}</p>
                                                                            <p class="mt-1">{summary}</p>
                                                                            <p class="mt-1 text-[10px] text-muted-foreground">
                                                                                {format!("{} · {}", event.actor, event.created_at)}
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
                                                            {format!("{}: {}", tr(locale, "Updated", "Обновлено"), gate.updated_at)}
                                                        </p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !review_policy_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Moderation policy", "Политика модерации")}
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
                                                {tr(locale, "Next actions", "Следующие действия")}
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
                                                {tr(locale, "Operator commands", "Команды оператора")}
                                            </p>
                                            {operator_command_lines.clone().into_iter().map(|line| {
                                                let copy_label = tr(locale, "Copy", "Копировать");
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
                                                {tr(locale, "Live API actions", "Live API-действия")}
                                            </p>
                                            {live_api_action_lines.clone().into_iter().map(|item| {
                                                let copy_label = tr(locale, "Copy", "Копировать");
                                                let copy_curl_label = tr(locale, "Copy cURL", "Копировать cURL");
                                                let copy_xtask_label = tr(locale, "Copy xtask", "Копировать xtask");
                                                let line_for_copy = item.endpoint.clone();
                                                let curl_snippet = curl_snippet_for_live_api_action(&item);
                                                let curl_for_copy = curl_snippet.clone();
                                                let xtask_for_copy = item.xtask_hint.clone();
                                                let authority_label = tr(locale, "Allowed actor", "Кто может вызывать");
                                                let body_label = tr(locale, "Request body", "Тело запроса");
                                                let headers_label = tr(locale, "Headers", "Заголовки");
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
                                    <Show when=move || can_validate || can_approve || can_reject || can_transfer_owner || can_yank>
                                        <div class="mt-3 space-y-3 rounded-lg border border-border bg-background p-3">
                                            <div class="flex flex-wrap items-center justify-between gap-3">
                                                <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                    {tr(locale, "Interactive actions", "Интерактивные действия")}
                                                </p>
                                                <label class="inline-flex items-center gap-2 text-xs text-muted-foreground">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || governance_dry_run.get()
                                                        on:change=move |event| {
                                                            let next = event_target_checked(&event);
                                                            set_governance_dry_run.set(next);
                                                            if next {
                                                                set_governance_confirmation_action.set(None);
                                                                set_governance_feedback.set(None);
                                                            }
                                                        }
                                                    />
                                                    <span>{tr(locale, "Dry run", "Dry run")}</span>
                                                </label>
                                            </div>
                                            <div class="grid gap-3 lg:grid-cols-2">
                                                <Input
                                                    value=Signal::derive(move || governance_actor.get())
                                                    set_value=set_governance_actor
                                                    placeholder=tr(locale, "governance:platform", "governance:platform")
                                                    label=tr(locale, "Actor", "Actor")
                                                />
                                                <Input
                                                    value=Signal::derive(move || governance_publisher.get())
                                                    set_value=set_governance_publisher
                                                    placeholder=tr(locale, "publisher:team", "publisher:team")
                                                    label=tr(locale, "Publisher (optional)", "Publisher (необязательно)")
                                                />
                                                <Input
                                                    value=Signal::derive(move || governance_new_owner_actor.get())
                                                    set_value=set_governance_new_owner_actor
                                                    placeholder=tr(locale, "publisher:new-owner", "publisher:new-owner")
                                                    label=tr(locale, "New owner actor", "Новый владелец")
                                                />
                                                <div class="flex flex-col gap-2">
                                                    <label class="text-sm font-medium leading-none">
                                                        {tr(locale, "Reason", "Причина")}
                                                    </label>
                                                    <textarea
                                                        class="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                                                        prop:value=move || governance_reason.get()
                                                        placeholder=tr(locale, "Required for reject, owner transfer, and yank.", "Обязательно для reject, owner transfer и yank.")
                                                        on:input=move |event| {
                                                            set_governance_reason.set(event_target_value(&event));
                                                        }
                                                    ></textarea>
                                                </div>
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get() || !can_validate)
                                                    on_click=on_validate_request
                                                >
                                                    {tr(locale, "Validate", "Validate")}
                                                </Button>
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get() || !can_approve)
                                                    on_click=on_approve_request
                                                >
                                                    {tr(locale, "Approve", "Approve")}
                                                </Button>
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get() || !can_reject)
                                                    on_click=on_reject_request
                                                >
                                                    {move || {
                                                        if !governance_dry_run.get()
                                                            && governance_confirmation_action.get().as_deref()
                                                                == Some("reject")
                                                        {
                                                            tr(locale, "Confirm reject", "Подтвердить отклонение")
                                                        } else {
                                                            tr(locale, "Reject", "Reject")
                                                        }
                                                    }}
                                                </Button>
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get() || !can_transfer_owner)
                                                    on_click=on_transfer_owner
                                                >
                                                    {move || {
                                                        if !governance_dry_run.get()
                                                            && governance_confirmation_action.get().as_deref()
                                                                == Some("owner-transfer")
                                                        {
                                                            tr(locale, "Confirm owner transfer", "Подтвердить передачу")
                                                        } else {
                                                            tr(locale, "Owner transfer", "Owner transfer")
                                                        }
                                                    }}
                                                </Button>
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get() || !can_yank)
                                                    on_click=on_yank_release
                                                >
                                                    {move || {
                                                        if !governance_dry_run.get()
                                                            && governance_confirmation_action.get().as_deref()
                                                                == Some("yank")
                                                        {
                                                            tr(locale, "Confirm yank", "Подтвердить отзыв")
                                                        } else {
                                                            tr(locale, "Yank", "Yank")
                                                        }
                                                    }}
                                                </Button>
                                                <Button
                                                    class="h-8 px-3 py-1 text-xs"
                                                    disabled=Signal::derive(move || governance_submitting.get())
                                                    on_click=Callback::new(move |_| {
                                                        set_governance_confirmation_action.set(None);
                                                        set_governance_feedback.set(None);
                                                        on_refresh_detail.run(())
                                                    })
                                                >
                                                    {tr(locale, "Refresh", "Обновить")}
                                                </Button>
                                            </div>
                                            <Show when=move || governance_confirmation_action.get().is_some() && !governance_dry_run.get()>
                                                <div class="space-y-3 rounded-md border border-amber-300 bg-amber-50 px-3 py-3 text-xs text-amber-900">
                                                    <p class="font-medium">
                                                        {move || governance_feedback.get().unwrap_or_default()}
                                                    </p>
                                                    <div class="flex flex-wrap gap-2">
                                                        <Button
                                                            class="h-8 px-3 py-1 text-xs"
                                                            disabled=Signal::derive(move || governance_submitting.get())
                                                            on_click=Callback::new(move |ev| {
                                                                match governance_confirmation_action.get().as_deref() {
                                                                    Some("reject") => on_reject_request.run(ev),
                                                                    Some("owner-transfer") => on_transfer_owner.run(ev),
                                                                    Some("yank") => on_yank_release.run(ev),
                                                                    _ => {}
                                                                }
                                                            })
                                                        >
                                                            {move || governance_confirmation_action
                                                                .get()
                                                                .map(|action| destructive_governance_action_label(&action, locale).to_string())
                                                                .unwrap_or_default()}
                                                        </Button>
                                                        <Button
                                                            class="h-8 px-3 py-1 text-xs"
                                                            disabled=Signal::derive(move || governance_submitting.get())
                                                            on_click=Callback::new(move |_| {
                                                                set_governance_confirmation_action.set(None);
                                                                set_governance_feedback.set(None);
                                                            })
                                                        >
                                                            {tr(locale, "Cancel", "Отмена")}
                                                        </Button>
                                                    </div>
                                                </div>
                                            </Show>
                                            <Show when=move || governance_submitting.get()>
                                                <div class="rounded-md border border-border bg-background/80 px-3 py-2 text-xs text-muted-foreground">
                                                    {tr(locale, "Submitting registry governance action...", "Отправка registry governance-действия...")}
                                                </div>
                                            </Show>
                                            <Show when=move || governance_feedback.get().is_some()>
                                                <div class="rounded-md border border-emerald-300 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">
                                                    {move || governance_feedback.get().unwrap_or_default()}
                                                </div>
                                            </Show>
                                            <Show when=move || governance_error.get().is_some()>
                                                <div class="rounded-md border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-700">
                                                    {move || governance_error.get().unwrap_or_default()}
                                                </div>
                                            </Show>
                                            <Show when=move || governance_result.get().is_some()>
                                                <div class="space-y-2 rounded-md border border-border bg-background/80 px-3 py-2 text-xs text-muted-foreground">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class="font-medium text-card-foreground">
                                                            {move || governance_result.get().map(|result| result.action).unwrap_or_default()}
                                                        </span>
                                                        <span>
                                                            {move || governance_result.get().and_then(|result| result.status).map(|status| humanize_token(&status)).unwrap_or_default()}
                                                        </span>
                                                    </div>
                                                    <Show when=move || governance_result.get().is_some_and(|result| !result.warnings.is_empty())>
                                                        <div class="space-y-1">
                                                            <p class="text-[11px] uppercase tracking-wide text-muted-foreground">
                                                                {tr(locale, "Warnings", "Предупреждения")}
                                                            </p>
                                                            {move || governance_result
                                                                .get()
                                                                .map(|result| result.warnings.into_iter().map(|warning| {
                                                                    view! {
                                                                        <div class="rounded border border-amber-200 bg-amber-50 px-2 py-1 text-[11px] text-amber-800">
                                                                            {warning}
                                                                        </div>
                                                                    }
                                                                }).collect_view())
                                                                .unwrap_or_default()}
                                                        </div>
                                                    </Show>
                                                    <Show when=move || governance_result.get().is_some_and(|result| !result.errors.is_empty())>
                                                        <div class="space-y-1">
                                                            <p class="text-[11px] uppercase tracking-wide text-muted-foreground">
                                                                {tr(locale, "Errors", "Ошибки")}
                                                            </p>
                                                            {move || governance_result
                                                                .get()
                                                                .map(|result| result.errors.into_iter().map(|error| {
                                                                    view! {
                                                                        <div class="rounded border border-red-200 bg-red-50 px-2 py-1 text-[11px] text-red-700">
                                                                            {error}
                                                                        </div>
                                                                    }
                                                                }).collect_view())
                                                                .unwrap_or_default()}
                                                        </div>
                                                    </Show>
                                                    {move || governance_result.get().and_then(|result| result.next_step).map(|next_step| view! {
                                                        <div>
                                                            <p class="text-[11px] uppercase tracking-wide text-muted-foreground">
                                                                {tr(locale, "Next step", "Следующий шаг")}
                                                            </p>
                                                            <p class="mt-1 text-[11px] text-muted-foreground">{next_step}</p>
                                                        </div>
                                                    })}
                                                </div>
                                            </Show>
                                        </div>
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
                                        <div class="mt-4 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Audit trail", "Аудит-след")}
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
                                                            <span>{format!("{}: {}", tr(locale, "Actor", "Актор"), actor)}</span>
                                                            {publisher.map(|publisher| {
                                                                view! {
                                                                    <span>{format!("{}: {}", tr(locale, "Publisher", "Издатель"), publisher)}</span>
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
                                    <div class="rounded-lg border border-border bg-background/70 p-4">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Registry readiness", "Готовность к registry")}
                                            </p>
                                            <span class=metadata_status_badge_classes(if metadata_required_issues > 0 { "warn" } else { "ready" })>
                                                {if metadata_required_issues > 0 {
                                                    format!("{} required issue(s)", metadata_required_issues)
                                                } else {
                                                    tr(locale, "No required metadata gaps", "Обязательных пробелов в метаданных нет").to_string()
                                                }}
                                            </span>
                                            <span class=metadata_status_badge_classes(if metadata_recommended_gaps > 0 { "warn" } else { "ready" })>
                                                {if metadata_recommended_gaps > 0 {
                                                    format!("{} recommended gap(s)", metadata_recommended_gaps)
                                                } else {
                                                    tr(locale, "Recommended visuals look complete", "Рекомендуемые визуальные материалы заполнены").to_string()
                                                }}
                                            </span>
                                            <span class=metadata_status_badge_classes("info")>
                                                {format!("{} ready signal(s)", metadata_ready_count)}
                                            </span>
                                        </div>
                                        <div class="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                                            {metadata_checklist.clone().into_iter().map(|item| {
                                                view! {
                                                    <div class=format!(
                                                        "rounded-lg border p-3 text-sm {}",
                                                        metadata_status_panel_classes(item.state)
                                                    )>
                                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                                            <p class="font-medium text-card-foreground">{item.label}</p>
                                                            <span class=metadata_status_badge_classes(item.state)>
                                                                {item.summary}
                                                            </span>
                                                        </div>
                                                        <p class="mt-2 text-xs text-muted-foreground">{item.detail}</p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                        <p class="mt-3 text-xs text-muted-foreground">
                                            {if module.source.eq_ignore_ascii_case("path") {
                                                tr(locale, "Workspace path modules can stay unpublished; this checklist is meant to surface what is already registry-ready versus what still needs operator follow-up.", "Workspace path-модули могут оставаться неопубликованными; этот checklist показывает, что уже готово для registry, а что ещё требует внимания оператора.")
                                            } else {
                                                tr(locale, "Registry-backed modules should ideally arrive here with the required metadata already satisfied.", "Registry-backed модули в идеале должны приходить сюда уже с заполненными обязательными метаданными.")
                                            }}
                                        </p>
                                    </div>
                                </Show>

                                {if has_marketplace_visuals {
                                    view! {
                                        <div class="rounded-lg border border-border bg-background/70 p-4">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Marketplace visuals", "Визуалы marketplace")}
                                            </p>
                                            <div class="mt-3 space-y-3">
                                                {module_banner_url_for_body.clone().map(|banner_url| {
                                                    let module_name = module_name.clone();
                                                    view! {
                                                        <div class="space-y-2">
                                                            <p class="text-xs text-muted-foreground">{tr(locale, "Banner", "Баннер")}</p>
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
                                                            <p class="text-xs text-muted-foreground">{tr(locale, "Screenshots", "Скриншоты")}</p>
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
                                                {tr(locale, "Tenant settings", "Настройки tenant")}
                                            </p>
                                            <p class="text-sm text-muted-foreground">
                                                {if settings_form_supported.get() {
                                                    tr(locale, "This module exposes schema-driven tenant settings from rustok-module.toml.", "Этот модуль публикует schema-driven tenant-настройки из rustok-module.toml.")
                                                } else if settings_editable.get() {
                                                    tr(locale, "Persist raw JSON settings for the current tenant. The payload is stored in tenant_modules.settings.", "Сохраняйте raw JSON-настройки для текущего tenant. Payload хранится в tenant_modules.settings.")
                                                } else {
                                                    tr(locale, "Enable this module for the current tenant before saving settings.", "Включите этот модуль для текущего tenant перед сохранением настроек.")
                                                }}
                                            </p>
                                        </div>
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                                            disabled=move || !settings_editable.get() || settings_saving.get()
                                            on:click=move |_| on_save_settings.run(())
                                        >
                                            {move || if settings_saving.get() { tr(locale, "Saving...", "Сохранение...") } else { tr(locale, "Save settings", "Сохранить настройки") }}
                                        </button>
                                    </div>
                                    <div class="mt-3 flex flex-wrap items-center gap-2 text-xs">
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                            {move || match tenant_module_for_body.get_value().as_ref() {
                                                Some(module) if module.enabled => tr(locale, "Tenant-enabled", "Включён для tenant").to_string(),
                                                Some(_) => tr(locale, "Tenant-disabled", "Выключен для tenant").to_string(),
                                                None if settings_editable.get() => tr(locale, "No tenant override yet", "Переопределения tenant пока нет").to_string(),
                                                None => tr(locale, "Unavailable until enabled", "Недоступно до включения").to_string(),
                                            }}
                                        </span>
                                        <Show when=move || settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                {format!(
                                                    "{} {}",
                                                    settings_schema_for_body.get_value().len(),
                                                    tr(locale, "fields", "полей")
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
                                                                                    <span>{tr(locale, "Enabled", "Включено")}</span>
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
                                                    tr(locale, "Editing schema-driven settings for", "Редактирование schema-driven настроек для"),
                                                    selected_slug_for_body.get_value(),
                                                    tr(locale, "Complex fields accept JSON per field.", "Сложные поля принимают JSON по каждому полю.")
                                                )
                                            } else {
                                                format!(
                                                    "{} `{}`.",
                                                    tr(locale, "Editing raw JSON settings for", "Редактирование raw JSON-настроек для"),
                                                    selected_slug_for_body.get_value()
                                                )
                                            }
                                        }}
                                    </p>
                                </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4">
                                    <div class="flex items-center gap-2">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Version history", "История версий")}
                                        </p>
                                        <Show when=move || loading.get()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                {tr(locale, "Refreshing", "Обновление")}
                                            </span>
                                        </Show>
                                    </div>
                                    {if version_trail.is_empty() {
                                        view! {
                                            <p class="mt-3 text-sm text-muted-foreground">
                                                {tr(locale, "No version history has been published for this module yet.", "Для этого модуля история версий пока не опубликована.")}
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <div class="mt-3 space-y-3">
                                                {version_trail.into_iter().map(|version| {
                                                    let checksum = short_checksum(version.checksum_sha256.as_deref());
                                                    view! {
                                                        <div class="flex flex-col gap-2 rounded-lg border border-border px-3 py-3 text-sm">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("v{}", version.version)}
                                                                </span>
                                                                {version.yanked.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-destructive px-2.5 py-0.5 text-xs font-semibold text-destructive-foreground">
                                                                        {tr(locale, "Yanked", "Отозван")}
                                                                    </span>
                                                                })}
                                                                {version.signature_present.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                                        {tr(locale, "Signed", "Подписан")}
                                                                    </span>
                                                                })}
                                                                <span class="text-xs text-muted-foreground">
                    {version.published_at.unwrap_or_else(|| tr(locale, "Unknown", "Неизвестно").to_string())}
                                                                </span>
                                                            </div>
                                                            {version.changelog.map(|changelog| view! {
                                                                <p class="text-sm text-muted-foreground">{changelog}</p>
                                                            })}
                                                            {checksum.map(|checksum| view! {
                                                                <div class="text-xs text-muted-foreground">
                                                                    <span class="font-mono">{format!("sha256 {}", checksum)}</span>
                                                                </div>
                                                            })}
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }}
                                </div>
                            </div>
                        }
                    })
                }}
            </Show>
        </div>
    }
}
