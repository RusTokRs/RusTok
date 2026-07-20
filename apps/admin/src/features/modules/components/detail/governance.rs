use super::{humanize_token, short_checksum, tr};
use crate::Locale;
use crate::entities::module::MarketplaceModule;
use crate::entities::module::model::{
    RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryOwnerLifecycle, RegistryPublishRequestLifecycle, RegistryReleaseLifecycle,
    RegistryValidationStageLifecycle,
};
use crate::features::modules::transport::RegistryMutationResult;

#[derive(Clone)]
pub struct RegistryLiveApiActionHint {
    pub endpoint: String,
    pub authority: String,
    pub note: Option<String>,
    pub body_hint: Option<String>,
    pub header_hint: Option<String>,
    pub xtask_hint: Option<String>,
    pub write_path: bool,
}

#[derive(Clone)]
pub struct RegistryAutomatedCheckItem {
    pub key: String,
    pub status: String,
    pub detail: String,
}

pub const REGISTRY_APPROVE_OVERRIDE_REASON_CODES: &[&str] = &[
    "manual_review_complete",
    "trusted_first_party",
    "expedited_release",
    "governance_override",
    "other",
];

pub fn registry_governance_hint(module: &MarketplaceModule, locale: Locale) -> String {
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

pub fn registry_request_status_badge_classes(status: &str) -> &'static str {
    if status_eq(status, "published") || status_eq(status, "active") {
        "inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground"
    } else if status_eq(status, "rejected") || status_eq(status, "yanked") {
        "inline-flex items-center rounded-full border border-red-300 bg-red-50 px-2.5 py-0.5 text-xs font-semibold text-red-700"
    } else {
        "inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground"
    }
}

pub fn validation_feedback_badge_classes(status: &str) -> &'static str {
    if status_eq(status, "passed") || status_eq(status, "succeeded") {
        "inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground"
    } else if status_eq(status, "failed")
        || status_eq(status, "blocked")
        || status_eq(status, "rejected")
    {
        "inline-flex items-center rounded-full border border-red-300 bg-red-50 px-2.5 py-0.5 text-xs font-semibold text-red-700"
    } else {
        "inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground"
    }
}

pub fn status_eq(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}

pub fn governance_action_available(
    actions: &[RegistryGovernanceActionLifecycle],
    key: &str,
) -> bool {
    actions
        .iter()
        .any(|action| action.key.eq_ignore_ascii_case(key))
}

pub fn governance_action_contract<'a>(
    actions: &'a [RegistryGovernanceActionLifecycle],
    key: &str,
) -> Option<&'a RegistryGovernanceActionLifecycle> {
    actions
        .iter()
        .find(|action| action.key.eq_ignore_ascii_case(key))
}

pub fn merge_governance_actions(
    primary: &[RegistryGovernanceActionLifecycle],
    secondary: &[RegistryGovernanceActionLifecycle],
) -> Vec<RegistryGovernanceActionLifecycle> {
    let mut merged = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for action in primary.iter().chain(secondary.iter()) {
        if seen.insert(action.key.to_ascii_lowercase()) {
            merged.push(action.clone());
        }
    }

    merged
}

pub fn governance_action_reason_required(
    actions: &[RegistryGovernanceActionLifecycle],
    key: &str,
) -> bool {
    governance_action_contract(actions, key).is_some_and(|action| action.reason_required)
}

pub fn governance_action_reason_code_required(
    actions: &[RegistryGovernanceActionLifecycle],
    key: &str,
) -> bool {
    governance_action_contract(actions, key).is_some_and(|action| action.reason_code_required)
}

pub fn governance_action_reason_codes(
    actions: &[RegistryGovernanceActionLifecycle],
    key: &str,
) -> Vec<String> {
    governance_action_contract(actions, key)
        .map(|action| action.reason_codes.clone())
        .unwrap_or_default()
}

pub fn governance_action_reason_code_validation_message(
    actions: &[RegistryGovernanceActionLifecycle],
    key: &str,
    reason_code: &str,
    locale: Locale,
) -> Option<String> {
    let reason_code = reason_code.trim();
    if reason_code.is_empty() {
        return None;
    }

    let allowed_codes = governance_action_reason_codes(actions, key);
    if allowed_codes.is_empty()
        || allowed_codes
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(reason_code))
    {
        return None;
    }

    Some(format!(
        "{} {}: {}.",
        governance_action_label(key, locale),
        tr(
            locale,
            "expects one of the advertised reason codes",
            "ожидает один из объявленных reason code"
        ),
        allowed_codes.join(", ")
    ))
}

pub fn governance_action_label(action_key: &str, locale: Locale) -> &'static str {
    match action_key {
        "validate" => tr(locale, "Validate", "Validate"),
        "approve" => tr(locale, "Approve", "Approve"),
        "request_changes" => tr(locale, "Request changes", "Запросить изменения"),
        "hold" => tr(locale, "Hold", "Поставить на hold"),
        "resume" => tr(locale, "Resume", "Возобновить"),
        "reject" => tr(locale, "Reject", "Reject"),
        "owner_transfer" => tr(locale, "Owner transfer", "Owner transfer"),
        "yank" => tr(locale, "Yank", "Yank"),
        _ => tr(locale, "governance action", "governance-действие"),
    }
}

pub fn governance_reason_code_placeholder(
    selected_action: Option<&str>,
    actions: &[RegistryGovernanceActionLifecycle],
    locale: Locale,
) -> String {
    let Some(action_key) = selected_action else {
        return tr(
            locale,
            "Select an action to see the allowed reason codes.",
            "Выберите действие, чтобы увидеть допустимые reason code.",
        )
        .to_string();
    };

    let reason_codes = governance_action_reason_codes(actions, action_key);
    if reason_codes.is_empty() {
        return tr(
            locale,
            "This action does not currently advertise reason codes.",
            "Для этого действия reason code сейчас не объявлены.",
        )
        .to_string();
    }

    reason_codes.join(" / ")
}

pub fn governance_reason_placeholder(
    selected_action: Option<&str>,
    actions: &[RegistryGovernanceActionLifecycle],
    locale: Locale,
) -> String {
    let Some(action_key) = selected_action else {
        return tr(
            locale,
            "Select an action to see whether a governance reason is required.",
            "Выберите действие, чтобы понять, нужен ли governance reason.",
        )
        .to_string();
    };

    if governance_action_reason_required(actions, action_key) {
        format!(
            "{} {}.",
            governance_action_label(action_key, locale),
            tr(
                locale,
                "requires a governance reason in live mode",
                "требует governance reason в live-режиме"
            )
        )
    } else {
        format!(
            "{} {}.",
            governance_action_label(action_key, locale),
            tr(
                locale,
                "does not require a governance reason unless the server asks for an override",
                "не требует governance reason, если только сервер отдельно не запросит override"
            )
        )
    }
}

pub fn governance_action_requirement_hint(
    selected_action: Option<&str>,
    actions: &[RegistryGovernanceActionLifecycle],
    locale: Locale,
) -> Option<String> {
    let action_key = selected_action?;
    let reason_required = governance_action_reason_required(actions, action_key);
    let reason_code_required = governance_action_reason_code_required(actions, action_key);
    let reason_codes = governance_action_reason_codes(actions, action_key);
    let requirement = match (reason_required, reason_code_required) {
        (true, true) => tr(
            locale,
            "Live mode requires both Reason and Reason code.",
            "В live-режиме нужны и Reason, и Reason code.",
        ),
        (true, false) => tr(
            locale,
            "Live mode requires Reason.",
            "В live-режиме нужен Reason.",
        ),
        (false, true) => tr(
            locale,
            "Live mode requires Reason code.",
            "В live-режиме нужен Reason code.",
        ),
        (false, false) => tr(
            locale,
            "The server currently marks this action as optional for Reason/Reason code.",
            "Сейчас сервер считает Reason/Reason code для этого действия необязательными.",
        ),
    };
    let reason_code_hint = if reason_codes.is_empty() {
        String::new()
    } else {
        format!(
            " {}: {}.",
            tr(locale, "Allowed codes", "Допустимые коды"),
            reason_codes.join(", ")
        )
    };

    Some(format!(
        "{} {}{}",
        governance_action_label(action_key, locale),
        requirement,
        reason_code_hint
    ))
}

pub fn validation_stage_requires_approval_override(status: &str) -> bool {
    !status_eq(status, "passed")
}

pub fn approval_override_required(validation_stages: &[RegistryValidationStageLifecycle]) -> bool {
    validation_stages
        .iter()
        .any(|stage| validation_stage_requires_approval_override(&stage.status))
}

pub fn approval_override_stage_labels(
    validation_stages: &[RegistryValidationStageLifecycle],
    locale: Locale,
) -> Vec<String> {
    validation_stages
        .iter()
        .filter(|stage| validation_stage_requires_approval_override(&stage.status))
        .map(|stage| {
            format!(
                "{} ({})",
                follow_up_gate_label(&stage.key, locale),
                humanize_token(&stage.status)
            )
        })
        .collect()
}

pub fn approval_override_warning_lines(
    validation_stages: &[RegistryValidationStageLifecycle],
    locale: Locale,
) -> Vec<String> {
    let pending_stage_labels = approval_override_stage_labels(validation_stages, locale);
    if pending_stage_labels.is_empty() {
        return Vec::new();
    }

    vec![
        format!(
            "{}: {}.",
            tr(
                locale,
                "Live approve now requires an explicit override because these follow-up stages are not passed",
                "Для live approve теперь нужен явный override, потому что эти follow-up stages ещё не пройдены"
            ),
            pending_stage_labels.join(", ")
        ),
        format!(
            "{}: {}.",
            tr(
                locale,
                "Fill both Reason and Reason code before approving, or mark the remaining stages as passed first",
                "Перед approve заполните и Reason, и Reason code, либо сначала переведите оставшиеся stages в passed"
            ),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
        ),
    ]
}

pub fn validation_stage_has_local_xtask_runner(stage_key: &str) -> bool {
    matches!(
        stage_key,
        "compile_smoke" | "targeted_tests" | "security_policy_review"
    )
}

pub fn validation_stage_runner_xtask_hint(
    module_slug: &str,
    request_id: &str,
    stage_key: &str,
) -> String {
    if stage_key.eq_ignore_ascii_case("security_policy_review") {
        format!(
            "cargo xtask module stage-run {} {} {} --confirm-manual-review --detail \"Manual security/policy review completed.\" --registry-url <registry-url> --auth-token <token>",
            module_slug, request_id, stage_key
        )
    } else {
        format!(
            "cargo xtask module stage-run {} {} {} --registry-url <registry-url> --auth-token <token>",
            module_slug, request_id, stage_key
        )
    }
}

pub fn registry_mutation_result_summary(result: &RegistryMutationResult, locale: Locale) -> String {
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

pub fn destructive_governance_action_label(action: &str, locale: Locale) -> &'static str {
    match action {
        "reject" => tr(locale, "Reject", "Отклонить"),
        "owner-transfer" => tr(locale, "Owner transfer", "Передать владение"),
        "yank" => tr(locale, "Yank", "Отозвать"),
        _ => tr(locale, "Confirm action", "Подтвердить действие"),
    }
}

pub fn destructive_governance_confirmation_message(
    action: &str,
    module_slug: &str,
    release_version: Option<&str>,
    new_owner_user_id: Option<&str>,
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
            new_owner_user_id.unwrap_or("<new-owner-user-id>"),
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
            "Подтвердите live governance-действие.",
        )
        .to_string(),
    }
}

pub fn curl_snippet_for_live_api_action(item: &RegistryLiveApiActionHint) -> Option<String> {
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

pub fn lifecycle_detail_lines(
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(owner) = owner_binding {
        lines.push(format!(
            "{}: {} · {}: {}",
            tr(locale, "Owner", "Владелец"),
            owner.owner,
            tr(locale, "Bound by", "Привязал"),
            owner.bound_by
        ));
    } else {
        lines.push(
            tr(
                locale,
                "Owner: [No owner bound to this slug yet]",
                "Владелец: [Владелец для этого slug ещё не привязан]",
            )
            .to_string(),
        );
    }

    if let Some(release) = release {
        let checksum = short_checksum(release.checksum_sha256.as_deref());

        lines.push(format!(
            "{}: v{} · {}: {} · {}",
            tr(locale, "Release", "Релиз"),
            release.version,
            tr(locale, "Publisher", "Издатель"),
            release.publisher,
            checksum
                .map(|val| format!("sha256 {val}"))
                .unwrap_or_else(|| tr(locale, "No checksum", "Нет контрольной суммы").to_string())
        ));
    } else {
        lines.push(
            tr(
                locale,
                "Release: [No release published yet]",
                "Релиз: [Релиз ещё не опубликован]",
            )
            .to_string(),
        );
    }

    if let Some(request) = request {
        let mut request_line = format!(
            "{}: {} · {}: {}",
            tr(locale, "Publish request", "Publish-запрос"),
            request.id,
            tr(locale, "Status", "Статус"),
            humanize_token(&request.status)
        );
        if let Some(publisher) = &request.publisher {
            request_line.push_str(&format!(
                " · {}: {}",
                tr(locale, "Publisher", "Издатель"),
                publisher
            ));
        }
        lines.push(request_line);

        let mut meta_line = format!(
            "{}: {}",
            tr(locale, "Requested by", "Запросил"),
            request.requested_by
        );
        if let Some(approved_by) = &request.approved_by {
            meta_line.push_str(&format!(
                " · {}: {}",
                tr(locale, "Approved by", "Одобрил"),
                approved_by
            ));
        }
        if let Some(rejected_by) = &request.rejected_by {
            meta_line.push_str(&format!(
                " · {}: {}",
                tr(locale, "Rejected by", "Отклонил"),
                rejected_by
            ));
        }
        lines.push(meta_line);

        if let Some(reason) = &request.rejection_reason {
            lines.push(format!(
                "{}: {reason}",
                tr(locale, "Rejection reason", "Причина отклонения")
            ));
        }
    }
    lines
}

pub fn is_validation_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "validation_queued" | "validation_passed" | "validation_failed"
    )
}

pub fn latest_validation_event(
    events: &[RegistryGovernanceEventLifecycle],
) -> Option<&RegistryGovernanceEventLifecycle> {
    events
        .iter()
        .find(|event| is_validation_event_type(&event.event_type))
}

pub fn is_validation_job_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "validation_job_queued"
            | "validation_job_started"
            | "validation_job_succeeded"
            | "validation_job_failed"
    )
}

pub fn latest_validation_job_event(
    events: &[RegistryGovernanceEventLifecycle],
) -> Option<&RegistryGovernanceEventLifecycle> {
    events
        .iter()
        .find(|event| is_validation_job_event_type(&event.event_type))
}

#[allow(dead_code)]
pub fn governance_detail_automated_checks(
    details: &serde_json::Value,
) -> Vec<RegistryAutomatedCheckItem> {
    details
        .get("automated_checks")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?.trim();
            let status = item.get("status")?.as_str()?.trim();
            let detail = item.get("detail")?.as_str()?.trim();
            if key.is_empty() || status.is_empty() || detail.is_empty() {
                return None;
            }
            Some(RegistryAutomatedCheckItem {
                key: key.to_string(),
                status: status.to_string(),
                detail: detail.to_string(),
            })
        })
        .collect()
}

pub fn automated_check_label(key: &str, locale: Locale) -> String {
    match key {
        "artifact_bundle_contract" => tr(
            locale,
            "Artifact bundle contract",
            "Artifact bundle contract",
        )
        .to_string(),
        _ => humanize_token(key),
    }
}

pub fn validation_job_event_context_lines(
    event: &RegistryGovernanceEventLifecycle,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(attempt_number) = governance_detail_i64(&event.payload, "attempt_number") {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Attempt", "Attempt"),
            attempt_number
        ));
    }
    if let Some(detail) = governance_detail_string(&event.payload, "detail") {
        lines.push(format!("{}: {}", tr(locale, "Detail", "Detail"), detail));
    }
    if !event.payload.errors.is_empty() {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Error", "Error"),
            event.payload.errors.join("; ")
        ));
    }
    lines
}

pub fn latest_governance_event_of_types<'a>(
    events: &'a [RegistryGovernanceEventLifecycle],
    event_types: &[&str],
) -> Option<&'a RegistryGovernanceEventLifecycle> {
    events.iter().find(|event| {
        event_types
            .iter()
            .any(|event_type| event.event_type.eq_ignore_ascii_case(event_type))
    })
}

pub fn registry_request_is_review_ready(request: &RegistryPublishRequestLifecycle) -> bool {
    status_eq(&request.status, "approved")
}

pub fn registry_validation_outcome_summary(
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

pub fn follow_up_gate_label(key: &str, locale: Locale) -> String {
    match key {
        "compile_smoke" => tr(locale, "Compile smoke", "Compile smoke").to_string(),
        "targeted_tests" => tr(locale, "Targeted tests", "Targeted tests").to_string(),
        "security_policy_review" => {
            tr(locale, "Security/policy review", "Security/policy review").to_string()
        }
        _ => humanize_token(key),
    }
}

pub fn registry_review_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> String {
    let operators = tr(
        locale,
        "operators with MODULES_MANAGE",
        "операторы с MODULES_MANAGE",
    );
    owner_binding
        .map(|owner| format!("{} / {}", owner.owner, operators,))
        .unwrap_or_else(|| operators.to_string())
}

pub fn registry_manage_publish_authority_label(
    request: &RegistryPublishRequestLifecycle,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> String {
    let operators = tr(
        locale,
        "operators with MODULES_MANAGE",
        "операторы с MODULES_MANAGE",
    );
    if let Some(owner) = owner_binding {
        return format!("{} / {}", owner.owner, operators,);
    }

    let mut actors = vec![request.requested_by.clone()];
    actors.push(operators.to_string());
    actors.join(" / ")
}

pub fn registry_owner_transfer_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> String {
    let operators = tr(
        locale,
        "operators with MODULES_MANAGE",
        "операторы с MODULES_MANAGE",
    );
    owner_binding
        .map(|owner| format!("{} / {}", owner.owner, operators))
        .unwrap_or_else(|| operators.to_string())
}

pub fn registry_yank_authority_label(
    owner_binding: Option<&RegistryOwnerLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    request: Option<&RegistryPublishRequestLifecycle>,
    locale: Locale,
) -> String {
    let mut actors = Vec::new();
    let operators = tr(
        locale,
        "operators with MODULES_MANAGE",
        "операторы с MODULES_MANAGE",
    );
    if let Some(owner) = owner_binding {
        actors.push(owner.owner.clone());
    }
    if let Some(release) = release {
        if !actors.iter().any(|actor| actor == &release.publisher) {
            actors.push(release.publisher.clone());
        }
    } else if let Some(request) = request.and_then(|request| request.publisher.clone()) {
        if !actors.iter().any(|actor| actor == &request) {
            actors.push(request);
        }
    }
    actors.push(operators.to_string());
    actors.join(" / ")
}

pub fn follow_up_gate_status_summary(
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
        "{}: {} | {}: {} | {}: {} | {}: {} | {}: {}",
        tr(locale, "Pending", "В ожидании"),
        pending,
        tr(locale, "Running", "В работе"),
        running,
        tr(locale, "Passed", "Пройдено"),
        passed,
        tr(locale, "Failed", "Провалено"),
        failed,
        tr(locale, "Blocked", "Заблокировано"),
        blocked
    );
    Some(summary)
}

pub fn validation_stage_status_summary(
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
        "{}: {} | {}: {} | {}: {} | {}: {} | {}: {}",
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

pub fn registry_review_policy_lines(
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!(
        "{}: {}",
        tr(locale, "Review authority", "Кто ревьюит"),
        registry_review_authority_label(owner_binding, locale)
    ));

    if owner_binding.is_none() {
        lines.push(
            tr(
                locale,
                "No persisted owner binding yet; the first publish is still controlled by the authenticated requester or an operator with MODULES_MANAGE.",
                "Сохранённой привязки владельца пока нет; первый publish всё ещё требует governance/bootstrap-обработки, прежде чем review станет owner-driven.",
            )
            .to_string(),
        );
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
                    "Request is ready for owner or MODULES_MANAGE review; requester and recorded publisher do not grant review access by themselves.",
                    "Запрос готов к review у владельца или операторов с MODULES_MANAGE; requester и записанный publisher больше не означают право на self-review.",
                )
                .to_string(),
            ),
            status if status_eq(status, "changes_requested") => lines.push(
                tr(
                    locale,
                    "Changes were requested; upload a fresh artifact revision before validation and review can continue.",
                    "Запрошены изменения; загрузите новый artifact revision, прежде чем продолжать validation и review.",
                )
                .to_string(),
            ),
            status if status_eq(status, "on_hold") => lines.push(
                tr(
                    locale,
                    "The request is explicitly on hold; validate/approve/reject should stay paused until a resume decision restores the previous lifecycle state.",
                    "Запрос явно поставлен на hold; validate/approve/reject должны оставаться на паузе, пока resume не вернёт предыдущее lifecycle-состояние.",
                )
                .to_string(),
            ),
            status if status_eq(status, "rejected") => lines.push(
                tr(
                    locale,
                    "Rejected requests should be fixed and recreated; moderation stays with the persisted owner or registry review actors.",
                    "Отклонённые запросы нужно исправлять и создавать заново; moderation остаётся у сохранённого владельца или операторов с MODULES_MANAGE.",
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
            && request.publisher.is_some()
            && request.publisher.as_ref() != owner_binding.map(|owner| &owner.owner)
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

pub fn registry_next_action_lines(
    module: &MarketplaceModule,
    request: Option<&RegistryPublishRequestLifecycle>,
    release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    validation_stages: &[RegistryValidationStageLifecycle],
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

    let xtask_prefix = "cargo xtask module";

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
            if approval_override_required(validation_stages) {
                lines.push(format!(
                    "{}: {}.",
                    tr(
                        locale,
                        "Before live approve, either close the remaining follow-up stages or send an explicit approval override",
                        "Перед live approve либо закройте оставшиеся follow-up stages, либо отправьте явный approval override"
                    ),
                    approval_override_stage_labels(validation_stages, locale).join(", ")
                ));
                lines.push(format!(
                    "{}: {}.",
                    tr(
                        locale,
                        "Supported approval override reason codes",
                        "Допустимые reason code для approval override"
                    ),
                    REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
                ));
            }
            if let Some(owner) = owner_binding {
                lines.push(format!(
                    "{}: {}.",
                    tr(locale, "Review can now be finalized by", "Review теперь может завершить"),
                    owner.owner
                ));
            } else {
                lines.push(
                    tr(
                        locale,
                        "The request is approved, but there is still no persisted owner binding; approval by operators with MODULES_MANAGE remains the safe path.",
                        "Запрос approved, но сохранённой привязки владельца ещё нет; approval через операторов с MODULES_MANAGE остаётся безопасным путём.",
                    )
                    .to_string(),
                );
            }
        }
        Some(status) if status_eq(status, "changes_requested") => lines.push(
            tr(
                locale,
                "Upload a fresh artifact revision next; request-changes keeps the same publish request alive, but review stays blocked until the new artifact is validated again.",
                "Следующий шаг — загрузить новый artifact revision; request-changes сохраняет тот же publish request, но review остаётся заблокированным, пока новый артефакт снова не пройдёт validation.",
            )
            .to_string(),
        ),
        Some(status) if status_eq(status, "on_hold") => lines.push(
            tr(
                locale,
                "The request is on hold; resume it explicitly when the blocking condition is cleared.",
                "Запрос находится на hold; явно возобновите его, когда блокирующее условие будет снято.",
            )
            .to_string(),
        ),
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
        _ => {}
    }

    if owner_binding.is_some()
        && request
            .and_then(|request| request.publisher.as_ref())
            .zip(owner_binding.map(|owner| owner.owner.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(format!(
            "{}: {} owner-transfer {} <new-owner-user-id> --dry-run {}",
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

pub fn registry_operator_command_lines(
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
        "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
        module.slug
    );

    match request.map(|request| request.status.as_str()) {
        None => lines.push(publish_dry_run.clone()),
        Some(status) if status_eq(status, "draft") => lines.push(publish_live),
        Some(status) if status_eq(status, "changes_requested") => lines.push(publish_live),
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
            .and_then(|request| request.publisher.as_ref())
            .zip(owner_binding.map(|owner| owner.owner.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(format!(
            "cargo xtask module owner-transfer {} <new-owner-user-id> --dry-run",
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
                if validation_stage_has_local_xtask_runner(&stage.key) {
                    let mut command =
                        validation_stage_runner_xtask_hint(&module.slug, &request.id, &stage.key);
                    command.push_str(" --dry-run");
                    lines.push(command);
                } else {
                    lines.push(format!(
                        "cargo xtask module stage {} {} <queued|running|passed|failed|blocked> --dry-run",
                        request.id, stage.key
                    ));
                }
            }
        }
    }

    lines.sort();
    lines.dedup();
    lines
}

pub fn registry_live_api_action_lines(
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

    let manage_publish_authority =
        registry_manage_publish_authority_label(request, owner_binding, locale);
    let bearer_header_hint = || "Authorization: Bearer <session-user-jwt>".to_string();

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
            authority: manage_publish_authority.clone(),
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
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
                module.slug
            )),
            write_path: true,
        });
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/hold", request.id),
            authority: registry_review_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Pause the request without rejecting it; live hold requires both a governance reason and a structured reason_code.",
                    "Поставить запрос на паузу без reject; live hold требует и governance reason, и structured reason_code.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
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
                    "Finalize a validated request into a published release. If follow-up validation stages are not all passed yet, include an explicit override reason and reason_code.",
                    "Финализирует провалидированный запрос в опубликованный релиз.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<override-reason-when-follow-up-stages-are-not-passed>\", \"reason_code\": \"manual_review_complete\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<override-reason-when-follow-up-stages-are-not-passed>\", \"reason_code\": \"manual_review_complete\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
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
                    "Reject requires both a governance reason and a structured reason_code in the request body.",
                    "Reject требует governance reason в теле запроса.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<governance-reason>\", \"reason_code\": \"policy_mismatch\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<governance-reason>\", \"reason_code\": \"policy_mismatch\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
            write_path: true,
        });
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/request-changes", request.id),
            authority: registry_review_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Request a fresh artifact revision without terminating the publish request; live request-changes requires both a governance reason and a structured reason_code.",
                    "Запросить новый artifact revision без завершения publish request; live request-changes требует и governance reason, и structured reason_code.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<change-request-reason>\", \"reason_code\": \"quality_gap\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<change-request-reason>\", \"reason_code\": \"quality_gap\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
            write_path: true,
        });
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/hold", request.id),
            authority: registry_review_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Pause the request without rejecting it; live hold requires both a governance reason and a structured reason_code.",
                    "Поставить запрос на паузу без reject; live hold требует и governance reason, и structured reason_code.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
            write_path: true,
        });
        for stage in validation_stages {
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
                    "{{ \"schema_version\": 1, \"dry_run\": false, \"stage\": \"{}\", \"status\": \"passed\", \"detail\": \"External validation recorded by operator.\", \"reason_code\": \"{}\", \"requeue\": false }}",
                    stage.key,
                    if stage.key.eq_ignore_ascii_case("security_policy_review") {
                        "manual_review_complete"
                    } else {
                        "local_runner_passed"
                    }
                )),
                header_hint: Some(bearer_header_hint()),
                xtask_hint: Some(if validation_stage_has_local_xtask_runner(&stage.key) {
                    validation_stage_runner_xtask_hint(&module.slug, &request.id, &stage.key)
                } else {
                    format!(
                        "cargo xtask module stage {} {} passed --detail \"External validation recorded by operator.\" --registry-url <registry-url> --auth-token <token>",
                        request.id, stage.key
                    )
                }),
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
    } else if status_eq(&request.status, "changes_requested") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("PUT /v2/catalog/publish/{}/artifact", request.id),
            authority: manage_publish_authority.clone(),
            note: Some(
                tr(
                    locale,
                    "Upload a fresh artifact revision to continue the same publish request after request-changes.",
                    "Загрузите новый artifact revision, чтобы продолжить тот же publish request после request-changes.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "<binary publish artifact body>",
                    "<binary publish artifact body>",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
                module.slug
            )),
            write_path: true,
        });
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/hold", request.id),
            authority: registry_review_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Pause the request without rejecting it; live hold requires both a governance reason and a structured reason_code.",
                    "Поставить запрос на паузу без reject; live hold требует и governance reason, и structured reason_code.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<hold-reason>\", \"reason_code\": \"release_window\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
            write_path: true,
        });
    } else if status_eq(&request.status, "on_hold") {
        lines.push(RegistryLiveApiActionHint {
            endpoint: format!("POST /v2/catalog/publish/{}/resume", request.id),
            authority: registry_review_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Resume the held request back into its previous lifecycle status; live resume requires both a governance reason and a structured reason_code.",
                    "Вернуть held request в предыдущее lifecycle-состояние; live resume требует и governance reason, и structured reason_code.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<resume-reason>\", \"reason_code\": \"review_complete\" }",
                    "{ \"schema_version\": 1, \"dry_run\": false, \"reason\": \"<resume-reason>\", \"reason_code\": \"review_complete\" }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: None,
            write_path: true,
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
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"version\": \"<version>\", \"reason\": \"<yank-reason>\", \"reason_code\": \"rollback\", \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"version\": \"<version>\", \"reason\": \"<yank-reason>\", \"reason_code\": \"rollback\", \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module yank {} {} --reason <yank-reason> --reason-code <security|legal|malware|critical_regression|rollback|other> --registry-url <registry-url> --auth-token <token>",
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
            authority: manage_publish_authority.clone(),
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
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
                module.slug
            )),
            write_path: true,
        });
    }

    if owner_binding.is_some()
        && request
            .publisher
            .as_ref()
            .zip(owner_binding.map(|owner| owner.owner.as_str()))
            .is_some_and(|(publisher, owner)| publisher != owner)
    {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/owner-transfer".to_string(),
            authority: registry_owner_transfer_authority_label(owner_binding, locale),
            note: Some(
                tr(
                    locale,
                    "Use this before treating a new requested publisher as the canonical owner; live owner transfer also requires a structured reason_code.",
                    "Используйте это до того, как считать нового requested publisher каноническим владельцем.",
                )
                .to_string(),
            ),
            body_hint: Some(
                tr(
                    locale,
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"new_owner_user_id\": \"<uuid>\", \"reason\": \"<transfer-reason>\", \"reason_code\": \"maintenance_handoff\", \"dry_run\": false }",
                    "{ \"schema_version\": 1, \"slug\": \"<module-slug>\", \"new_owner_user_id\": \"<uuid>\", \"reason\": \"<transfer-reason>\", \"reason_code\": \"maintenance_handoff\", \"dry_run\": false }",
                )
                .to_string(),
            ),
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module owner-transfer {} <new-owner-user-id> --reason <transfer-reason> --reason-code <maintenance_handoff|team_restructure|publisher_rotation|security_emergency|governance_override|other> --registry-url <registry-url> --auth-token <token>",
                module.slug
            )),
            write_path: true,
        });
    }

    if release.is_some_and(|release| status_eq(&release.status, "yanked")) {
        lines.push(RegistryLiveApiActionHint {
            endpoint: "POST /v2/catalog/publish".to_string(),
            authority: manage_publish_authority,
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
            header_hint: Some(bearer_header_hint()),
            xtask_hint: Some(format!(
                "cargo xtask module publish {} --registry-url <registry-url> --auth-token <token>",
                module.slug
            )),
            write_path: true,
        });
    }

    lines.sort_by(|left, right| left.endpoint.cmp(&right.endpoint));
    lines.dedup_by(|left, right| left.endpoint == right.endpoint);
    lines
}

pub fn governance_detail_string(
    payload: &RegistryGovernanceEventPayloadLifecycle,
    key: &str,
) -> Option<String> {
    let value = match key {
        "reason" => payload.reason.as_deref(),
        "reason_code" => payload.reason_code.as_deref(),
        "detail" => payload.detail.as_deref(),
        "version" => payload.version.as_deref(),
        "stage_key" => payload.stage_key.as_deref(),
        "mode" => payload.mode.as_deref(),
        "previous_owner" => payload
            .owner_transition
            .as_ref()
            .and_then(|value| value.previous_owner.as_deref()),
        "new_owner" => payload
            .owner_transition
            .as_ref()
            .and_then(|value| value.new_owner.as_deref()),
        "bound_by" => payload
            .owner_transition
            .as_ref()
            .and_then(|value| value.bound_by.as_deref()),
        _ => None,
    }?;

    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub fn governance_detail_string_list(
    payload: &RegistryGovernanceEventPayloadLifecycle,
    key: &str,
) -> Vec<String> {
    let values = match key {
        "warnings" => payload.warnings.clone(),
        "errors" => payload.errors.clone(),
        _ => Vec::new(),
    };

    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub fn governance_detail_i64(
    payload: &RegistryGovernanceEventPayloadLifecycle,
    key: &str,
) -> Option<i64> {
    match key {
        "attempt_number" => payload.attempt_number.map(i64::from),
        _ => None,
    }
}

pub fn governance_event_stage_key(event: &RegistryGovernanceEventLifecycle) -> Option<String> {
    governance_detail_string(&event.payload, "stage_key")
}

pub fn validation_stage_recent_history(
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

pub fn is_moderation_history_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "release_published"
            | "publish_approval_override"
            | "request_rejected"
            | "changes_requested"
            | "request_held"
            | "request_resumed"
            | "owner_transferred"
            | "release_yanked"
            | "validation_stage_running"
            | "validation_stage_started"
            | "validation_stage_passed"
            | "validation_stage_failed"
            | "validation_stage_blocked"
    )
}

pub fn moderation_history_events(
    events: &[RegistryGovernanceEventLifecycle],
    limit: usize,
) -> Vec<RegistryGovernanceEventLifecycle> {
    events
        .iter()
        .filter(|event| is_moderation_history_event_type(&event.event_type))
        .take(limit)
        .cloned()
        .collect()
}

pub fn moderation_history_badge_label(event_type: &str, locale: Locale) -> String {
    let event_type = match event_type {
        "validation_stage_started" => "validation_stage_running",
        other => other,
    };
    match event_type {
        "release_published" => tr(locale, "Approved", "Approved"),
        "publish_approval_override" => tr(locale, "Approval override", "Approval override"),
        "request_rejected" => tr(locale, "Rejected", "Rejected"),
        "changes_requested" => tr(locale, "Changes requested", "Запрошены изменения"),
        "request_held" => tr(locale, "On hold", "На hold"),
        "request_resumed" => tr(locale, "Resumed", "Возобновлён"),
        "owner_transferred" => tr(locale, "Owner transfer", "Owner transfer"),
        "release_yanked" => tr(locale, "Yanked", "Yanked"),
        "validation_stage_running" => tr(locale, "Stage running", "Stage running"),
        "validation_stage_passed" => tr(locale, "Stage passed", "Stage passed"),
        "validation_stage_failed" => tr(locale, "Stage failed", "Stage failed"),
        "validation_stage_blocked" => tr(locale, "Stage blocked", "Stage blocked"),
        _ => tr(locale, "Decision", "Decision"),
    }
    .to_string()
}

pub fn moderation_history_badge_status(event_type: &str) -> &'static str {
    match event_type {
        "release_published" => "published",
        "publish_approval_override" => "info",
        "request_rejected" => "rejected",
        "changes_requested" => "info",
        "request_held" => "blocked",
        "request_resumed" => "running",
        "release_yanked" => "yanked",
        "validation_stage_failed" => "failed",
        "validation_stage_blocked" => "blocked",
        "validation_stage_running" | "validation_stage_started" => "running",
        _ => "info",
    }
}

pub fn moderation_history_context_lines(
    event: &RegistryGovernanceEventLifecycle,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();
    let reason = governance_detail_string(&event.payload, "reason");
    let reason_code = governance_detail_string(&event.payload, "reason_code");
    let detail = governance_detail_string(&event.payload, "detail");
    let version = governance_detail_string(&event.payload, "version");
    let stage_key = governance_event_stage_key(event);
    let attempt_number = governance_detail_i64(&event.payload, "attempt_number");
    let previous_owner = governance_detail_string(&event.payload, "previous_owner");
    let new_owner = governance_detail_string(&event.payload, "new_owner");

    if let Some(version) = version {
        lines.push(format!(
            "{}: v{}",
            tr(locale, "Version", "Version"),
            version
        ));
    }

    if let Some(stage_key) = stage_key {
        let mut line = format!(
            "{}: {}",
            tr(locale, "Stage", "Stage"),
            follow_up_gate_label(&stage_key, locale)
        );
        if let Some(attempt_number) = attempt_number {
            line.push_str(&format!(
                " · {} {}",
                tr(locale, "attempt", "attempt"),
                attempt_number
            ));
        }
        lines.push(line);
    }

    if let (Some(previous_owner), Some(new_owner)) = (previous_owner, new_owner) {
        lines.push(format!(
            "{}: {} -> {}",
            tr(locale, "Ownership", "Ownership"),
            previous_owner,
            new_owner
        ));
    }

    if let Some(reason) = reason {
        lines.push(format!("{}: {}", tr(locale, "Reason", "Reason"), reason));
    }

    if let Some(reason_code) = reason_code {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Reason code", "Reason code"),
            humanize_token(&reason_code)
        ));
    }

    if let Some(detail) = detail {
        if !lines.iter().any(|line| line.ends_with(&detail)) {
            lines.push(format!("{}: {}", tr(locale, "Detail", "Detail"), detail));
        }
    }

    lines
}

pub fn governance_event_title(event_type: &str, locale: Locale) -> String {
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
        "changes_requested" => tr(locale, "Changes requested", "Запрошены изменения"),
        "request_held" => tr(locale, "Request placed on hold", "Запрос поставлен на hold"),
        "request_resumed" => tr(locale, "Request resumed", "Запрос возобновлён"),
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
        "validation_job_queued" => tr(locale, "Validation job queued", "Validation job queued"),
        "validation_job_started" => tr(locale, "Validation job running", "Validation job running"),
        "validation_job_succeeded" => tr(
            locale,
            "Validation job succeeded",
            "Validation job succeeded",
        ),
        "validation_job_failed" => tr(locale, "Validation job failed", "Validation job failed"),
        _ => return humanize_token(event_type),
    }
    .to_string()
}

pub fn governance_event_summary(
    event: &RegistryGovernanceEventLifecycle,
    locale: Locale,
) -> String {
    let event_type = match event.event_type.as_str() {
        "validation_stage_started" => "validation_stage_running",
        other => other,
    };
    let version = governance_detail_string(&event.payload, "version");
    let reason = governance_detail_string(&event.payload, "reason");
    let reason_code = governance_detail_string(&event.payload, "reason_code");
    let publisher = event.publisher.clone();
    let owner_principal = governance_detail_string(&event.payload, "new_owner");
    let mode = governance_detail_string(&event.payload, "mode");
    let warnings = governance_detail_string_list(&event.payload, "warnings");
    let errors = governance_detail_string_list(&event.payload, "errors");
    let stage_key = governance_event_stage_key(event);
    let stage_label = stage_key
        .as_deref()
        .map(|value| follow_up_gate_label(value, locale))
        .unwrap_or_else(|| tr(locale, "Validation stage", "Этап валидации").to_string());
    let stage_attempt = governance_detail_i64(&event.payload, "attempt_number");
    let stage_detail = governance_detail_string(&event.payload, "detail");

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
        "validation_job_queued"
        | "validation_job_started"
        | "validation_job_succeeded"
        | "validation_job_failed" => {
            let status = match event_type {
                "validation_job_queued" => tr(locale, "queued", "queued"),
                "validation_job_started" => tr(locale, "is running", "is running"),
                "validation_job_succeeded" => tr(locale, "succeeded", "succeeded"),
                "validation_job_failed" => tr(locale, "failed", "failed"),
                _ => unreachable!(),
            };

            let mut parts = vec![format!(
                "{} {status}",
                tr(locale, "Validation job", "Validation job")
            )];
            if let Some(attempt) = governance_detail_i64(&event.payload, "attempt_number") {
                parts.push(format!("{} {}", tr(locale, "attempt", "attempt"), attempt));
            }
            if let Some(detail) = governance_detail_string(&event.payload, "detail") {
                parts.push(detail);
            }
            if !event.payload.errors.is_empty() {
                parts.push(event.payload.errors.join("; "));
            }
            parts.join(" · ")
        }
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
        "changes_requested" => reason
            .map(|value| {
                let prefix = reason_code
                    .as_deref()
                    .map(|code| {
                        format!(
                            "{} ({})",
                            tr(locale, "Changes requested", "Запрошены изменения"),
                            humanize_token(code)
                        )
                    })
                    .unwrap_or_else(|| {
                        tr(locale, "Changes requested", "Запрошены изменения").to_string()
                    });
                format!("{prefix}: {value}")
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Review requested a fresh artifact revision.",
                    "Review запросил новый artifact revision.",
                )
                .to_string()
            }),
        "request_held" => reason
            .map(|value| {
                let prefix = reason_code
                    .as_deref()
                    .map(|code| {
                        format!(
                            "{} ({})",
                            tr(locale, "On hold", "На hold"),
                            humanize_token(code)
                        )
                    })
                    .unwrap_or_else(|| tr(locale, "On hold", "На hold").to_string());
                format!("{prefix}: {value}")
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "The request was placed on hold.",
                    "Запрос был поставлен на hold.",
                )
                .to_string()
            }),
        "request_resumed" => {
            let resumed_to_status = governance_event_stage_key(event).unwrap_or_else(|| {
                tr(
                    locale,
                    "previous lifecycle state",
                    "предыдущее lifecycle-состояние",
                )
                .to_string()
            });
            match reason {
                Some(reason) => format!(
                    "{}: {} ({})",
                    tr(locale, "Resumed to", "Возобновлён до"),
                    humanize_token(&resumed_to_status),
                    reason
                ),
                None => format!(
                    "{}: {}",
                    tr(locale, "Resumed to", "Возобновлён до"),
                    humanize_token(&resumed_to_status)
                ),
            }
        }
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
        "publish_approval_override" => reason
            .map(|value| {
                let prefix = reason_code
                    .as_deref()
                    .map(|code| {
                        format!(
                            "{} ({})",
                            tr(locale, "Approval override", "Approval override"),
                            humanize_token(code)
                        )
                    })
                    .unwrap_or_else(|| {
                        tr(locale, "Approval override", "Approval override").to_string()
                    });
                format!("{prefix}: {value}")
            })
            .unwrap_or_else(|| {
                tr(
                    locale,
                    "Publish approval used an explicit follow-up gate override.",
                    "Publish approval used an explicit follow-up gate override.",
                )
                .to_string()
            }),
        "owner_bound" => {
            let label = match mode.as_deref() {
                Some("rebind") => tr(locale, "Owner rebound", "Владелец перевязан"),
                _ => tr(locale, "Owner bound", "Владелец привязан"),
            };
            owner_principal
                .map(|owner_principal| format!("{label}: {owner_principal}"))
                .unwrap_or_else(|| label.to_string())
        }
        "owner_transferred" => {
            let previous_owner = governance_detail_string(&event.payload, "previous_owner");
            let new_owner =
                governance_detail_string(&event.payload, "new_owner").or(owner_principal);
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

pub fn follow_up_gate_status_badge_classes(status: &str) -> &'static str {
    validation_feedback_badge_classes(status)
}

pub fn follow_up_gate_status_text(status: &str, locale: Locale) -> String {
    if status_eq(status, "passed") || status_eq(status, "succeeded") {
        tr(locale, "Passed", "Пройдена").to_string()
    } else if status_eq(status, "failed") {
        tr(locale, "Failed", "Ошибка проверки").to_string()
    } else if status_eq(status, "blocked") {
        tr(locale, "Blocked by prerequisite", "Заблокирована").to_string()
    } else if status_eq(status, "pending") {
        tr(locale, "Pending verification", "Ожидает выполнения").to_string()
    } else {
        humanize_token(status)
    }
}

pub fn follow_up_gate_detail_lines(
    gate: &RegistryFollowUpGateLifecycle,
    locale: Locale,
) -> Vec<String> {
    let mut lines = Vec::new();
    let status_label = follow_up_gate_status_text(&gate.status, locale);

    lines.push(format!(
        "{}: {} · {}: {}",
        tr(locale, "Gate", "Проверка"),
        follow_up_gate_label(&gate.key, locale),
        tr(locale, "Status", "Статус"),
        status_label
    ));

    lines.push(format!(
        "{}: {}",
        tr(locale, "At", "Время"),
        gate.updated_at
    ));

    if !gate.detail.is_empty() {
        lines.push(format!(
            "{}: {}",
            tr(locale, "Detail", "Детали"),
            gate.detail
        ));
    }

    lines
}

pub fn governance_event_type_label(event_type: &str, locale: Locale) -> &'static str {
    match event_type {
        "publish_requested" => tr(locale, "Publish requested", "Запрошена публикация"),
        "validation_queued" => tr(
            locale,
            "Validation queued",
            "Валидация поставлена в очередь",
        ),
        "validation_job_queued" => tr(
            locale,
            "Validation job queued",
            "Валидация job поставлен в очередь",
        ),
        "validation_job_started" => tr(locale, "Validation job started", "Валидация job запущен"),
        "validation_job_succeeded" => tr(
            locale,
            "Validation job succeeded",
            "Валидация job завершён успешно",
        ),
        "validation_job_failed" => tr(
            locale,
            "Validation job failed",
            "Валидация job завершён с ошибкой",
        ),
        "validation_passed" => tr(locale, "Validation passed", "Валидация пройдена"),
        "validation_failed" => tr(locale, "Validation failed", "Валидация завершилась ошибкой"),
        "publish_approved" => tr(locale, "Publish approved", "Публикация одобрена"),
        "publish_approval_override" => tr(
            locale,
            "Publish approved with override",
            "Публикация одобрена с override",
        ),
        "release_published" => tr(locale, "Release published", "Релиз опубликован"),
        "request_rejected" => tr(locale, "Request rejected", "Запрос отклонён"),
        "changes_requested" => tr(locale, "Changes requested", "Запрошены изменения"),
        "request_held" => tr(locale, "Request held", "Запрос поставлен на hold"),
        "request_resumed" => tr(locale, "Request resumed", "Запрос возобновлён"),
        "release_yanked" => tr(locale, "Release yanked", "Релиз отозван"),
        "owner_bound" => tr(locale, "Owner bound", "Владелец привязан"),
        "owner_transferred" => tr(locale, "Ownership transferred", "Владение передано"),
        _ => tr(locale, "Governance event", "Governance-событие"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::module::model::RegistryOwnerLifecycle;
    use serde_json::json;

    fn sample_owner(owner: &str) -> RegistryOwnerLifecycle {
        RegistryOwnerLifecycle {
            owner: owner.to_string(),
            bound_by: "user:00000000-0000-0000-0000-000000000001".to_string(),
            bound_at: "2026-04-05T10:00:00Z".to_string(),
            updated_at: "2026-04-05T10:00:00Z".to_string(),
        }
    }

    fn sample_request(status: &str, publisher: Option<&str>) -> RegistryPublishRequestLifecycle {
        RegistryPublishRequestLifecycle {
            id: "req_123".to_string(),
            status: status.to_string(),
            requested_by: "user:requester".to_string(),
            publisher: publisher.map(str::to_string),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            changes_requested_by: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            created_at: "2026-04-05T10:00:00Z".to_string(),
            updated_at: "2026-04-05T10:00:00Z".to_string(),
            published_at: None,
        }
    }

    fn sample_module() -> MarketplaceModule {
        MarketplaceModule {
            slug: "example-module".to_string(),
            name: "Example Module".to_string(),
            latest_version: "1.2.3".to_string(),
            description: "Example description".to_string(),
            source: "registry".to_string(),
            kind: "feature".to_string(),
            category: "catalog".to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: "rustok-example".to_string(),
            dependencies: Vec::new(),
            ownership: "first_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: Some("RusTok Labs".to_string()),
            checksum_sha256: None,
            signature_present: true,
            versions: Vec::new(),
            has_admin_ui: true,
            has_storefront_ui: false,
            ui_classification: "admin-only".to_string(),
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: false,
            installed_version: None,
            update_available: false,
        }
    }

    fn sample_event(
        event_type: &str,
        details: serde_json::Value,
    ) -> RegistryGovernanceEventLifecycle {
        RegistryGovernanceEventLifecycle {
            id: "evt_1".to_string(),
            event_type: event_type.to_string(),
            actor: "user:00000000-0000-0000-0000-000000000001".to_string(),
            publisher: None,
            payload: RegistryGovernanceEventPayloadLifecycle::from_details(&details),
            created_at: "2026-04-05T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn governance_detail_automated_checks_parses_only_valid_items() {
        let checks = governance_detail_automated_checks(&json!({
            "automated_checks": [
                {
                    "key": "artifact_bundle_contract",
                    "status": "passed",
                    "detail": "Bundle contract passed."
                },
                {
                    "key": "artifact_bundle_contract",
                    "status": "",
                    "detail": "Should be ignored."
                }
            ]
        }));

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].key, "artifact_bundle_contract");
        assert_eq!(checks[0].status, "passed");
        assert_eq!(checks[0].detail, "Bundle contract passed.");
    }

    #[test]
    fn validation_job_event_context_lines_include_trace_fields() {
        let event = sample_event(
            "validation_job_failed",
            json!({
                "job_id": "rvj_123",
                "attempt_number": 2,
                "queue_reason": "validation_resumed",
                "request_status": "rejected",
                "error": "checksum mismatch"
            }),
        );

        let lines = validation_job_event_context_lines(&event, Locale::en);

        assert!(lines.iter().any(|line| line == "Attempt: 2"));
        assert!(!lines.is_empty());
    }

    #[test]
    fn moderation_history_context_lines_include_reason_code() {
        let event = sample_event(
            "request_rejected",
            json!({
                "version": "1.2.3",
                "reason": "Ownership evidence is incomplete.",
                "reason_code": "ownership_mismatch"
            }),
        );

        let lines = moderation_history_context_lines(&event, Locale::en);

        assert!(lines.iter().any(|line| line == "Version: v1.2.3"));
        assert!(
            lines
                .iter()
                .any(|line| line == "Reason: Ownership evidence is incomplete.")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "Reason code: Ownership Mismatch")
        );
    }

    #[test]
    fn registry_review_policy_lines_drop_operator_override_copy() {
        let owner = sample_owner("owner:module");
        let lines = registry_review_policy_lines(None, None, Some(&owner), Locale::en);

        assert_eq!(
            lines.first().map(String::as_str),
            Some("Review authority: owner:module / operators with MODULES_MANAGE")
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("operators with MODULES_MANAGE may override"))
        );
    }

    #[test]
    fn owner_transfer_confirmation_uses_new_owner_user_id_contract() {
        let message = destructive_governance_confirmation_message(
            "owner-transfer",
            "example-module",
            None,
            Some("9a6b5c9e-0d3e-4b62-9f2f-c11d1aa6f12f"),
            Locale::en,
        );

        assert!(message.contains("example-module"));
        assert!(message.contains("9a6b5c9e-0d3e-4b62-9f2f-c11d1aa6f12f"));
        assert!(!message.contains("new-owner-actor"));
    }

    #[test]
    fn validation_stage_runner_hint_requires_auth_token() {
        let hint = validation_stage_runner_xtask_hint("example-module", "req_123", "compile_smoke");

        assert!(hint.contains("--registry-url <registry-url>"));
        assert!(hint.contains("--auth-token <token>"));
    }

    #[test]
    fn live_api_action_hints_use_bearer_auth_without_actor_headers() {
        let module = sample_module();
        let request = sample_request(
            "approved",
            Some("user:00000000-0000-0000-0000-000000000002"),
        );
        let owner = sample_owner("user:owner");

        let hints = registry_live_api_action_lines(
            &module,
            Some(&request),
            None,
            Some(&owner),
            &[],
            Locale::en,
        );

        let approve_hint = hints
            .iter()
            .find(|hint| hint.endpoint.ends_with("/approve"))
            .expect("approve hint");

        assert_eq!(
            approve_hint.header_hint.as_deref(),
            Some("Authorization: Bearer <session-user-jwt>")
        );
    }

    #[test]
    fn owner_transfer_hints_use_new_owner_user_id_contract() {
        let module = sample_module();
        let request = sample_request(
            "published",
            Some("user:00000000-0000-0000-0000-000000000002"),
        );
        let owner = sample_owner("user:owner");

        let api_hints = registry_live_api_action_lines(
            &module,
            Some(&request),
            None,
            Some(&owner),
            &[],
            Locale::en,
        );
        let owner_transfer_api_hint = api_hints
            .iter()
            .find(|hint| hint.endpoint == "POST /v2/catalog/owner-transfer")
            .expect("owner transfer api hint");
        let owner_transfer_cli_hint = owner_transfer_api_hint
            .xtask_hint
            .as_deref()
            .expect("owner transfer cli hint");

        assert!(
            owner_transfer_api_hint
                .body_hint
                .as_deref()
                .unwrap_or_default()
                .contains("\"new_owner_user_id\"")
        );
        assert!(owner_transfer_cli_hint.contains("<new-owner-user-id>"));
        assert!(owner_transfer_cli_hint.contains("--auth-token <token>"));
        assert!(!owner_transfer_cli_hint.contains("<new-owner-actor>"));

        let operator_hints =
            registry_operator_command_lines(&module, Some(&request), None, Some(&owner), &[]);
        let owner_transfer_operator_hint = operator_hints
            .iter()
            .find(|hint| hint.contains("owner-transfer"))
            .expect("owner transfer operator hint");
        assert!(owner_transfer_operator_hint.contains("<new-owner-user-id>"));
        assert!(!owner_transfer_operator_hint.contains("<new-owner-actor>"));
    }
}
