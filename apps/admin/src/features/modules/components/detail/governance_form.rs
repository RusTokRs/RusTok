use crate::Locale;
use crate::entities::module::model::RegistryGovernanceActionLifecycle;
use crate::features::modules::transport::{RegistryMutationResult, RegistryPublishStatusContract};
use crate::shared::ui::{Button, Input};
use leptos::prelude::*;

use super::{
    governance::{
        approval_override_warning_lines, destructive_governance_action_label,
        governance_action_available, governance_action_requirement_hint,
        governance_reason_code_placeholder, governance_reason_placeholder,
    },
    humanize_token, tr,
};

#[component]
pub fn GovernanceForm(
    locale: Locale,
    governance_dry_run: Signal<bool>,
    set_governance_dry_run: WriteSignal<bool>,
    governance_status_contract: Signal<Option<RegistryPublishStatusContract>>,
    governance_status_contract_loading: Signal<bool>,
    governance_status_contract_error: Signal<Option<String>>,
    has_request_status_contract: bool,
    governance_new_owner_user_id: Signal<String>,
    set_governance_new_owner_user_id: WriteSignal<String>,
    governance_reason_code: Signal<String>,
    set_governance_reason_code: WriteSignal<String>,
    governance_reason: Signal<String>,
    set_governance_reason: WriteSignal<String>,
    governance_intent_action: Signal<Option<String>>,
    set_governance_intent_action: WriteSignal<Option<String>>,
    governance_actions_for_form: Signal<Vec<RegistryGovernanceActionLifecycle>>,
    governance_submitting: Signal<bool>,
    governance_confirmation_action: Signal<Option<String>>,
    set_governance_confirmation_action: WriteSignal<Option<String>>,
    governance_feedback: Signal<Option<String>>,
    set_governance_feedback: WriteSignal<Option<String>>,
    governance_error: Signal<Option<String>>,
    governance_result: Signal<Option<RegistryMutationResult>>,
    on_validate: Callback<()>,
    on_approve: Callback<()>,
    on_request_changes: Callback<()>,
    on_hold: Callback<()>,
    on_resume: Callback<()>,
    on_reject: Callback<()>,
    on_transfer_owner: Callback<()>,
    on_yank_release: Callback<()>,
    on_refresh: Callback<()>,
) -> impl IntoView {
    let _ = set_governance_intent_action;

    view! {
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
            <Show when=move || governance_status_contract.get().is_some_and(|status| status.approval_override_required)>
                <div class="space-y-2 rounded-md border border-amber-300 bg-amber-50 px-3 py-3 text-xs text-amber-900">
                    <p class="font-medium">
                        {tr(locale, "Approval override required", "Нужен approval override")}
                    </p>
                    <ul class="list-disc space-y-1 pl-4">
                        {move || governance_status_contract
                            .get()
                            .filter(|status| status.approval_override_required)
                            .map(|status| approval_override_warning_lines(&status.validation_stages, locale))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|line| view! { <li>{line}</li> })
                            .collect_view()}
                    </ul>
                </div>
            </Show>
            <Show when=move || governance_status_contract_loading.get() || governance_status_contract_error.get().is_some() || governance_status_contract.get().is_some() || has_request_status_contract>
                <div class="rounded-md border border-border bg-background/80 px-3 py-2 text-xs text-muted-foreground">
                    {move || {
                        if governance_status_contract_loading.get() {
                            return tr(
                                locale,
                                "Refreshing authenticated request status contract...",
                                "Обновляется аутентифицированный контракт статуса запроса...",
                            )
                            .to_string();
                        }
                        if let Some(error) = governance_status_contract_error.get() {
                            return format!(
                                "{} {}",
                                tr(
                                    locale,
                                    "Authenticated request status is unavailable; request-level actions stay disabled until the fetch succeeds.",
                                    "Аутентифицированный статус запроса недоступен; request-level действия останутся выключенными, пока fetch не пройдет.",
                                ),
                                error
                            );
                        }
                        if let Some(status) = governance_status_contract.get() {
                            return format!(
                                "{}: {}{}",
                                tr(locale, "Authenticated request contract", "Аутентифицированный контракт запроса"),
                                humanize_token(&status.status),
                                status
                                    .next_step
                                    .as_ref()
                                    .map(|next_step| format!(" · {}", next_step))
                                    .unwrap_or_default()
                            );
                        }
                        tr(
                            locale,
                            "Sign in with a session-backed user token to load the authoritative request-level governance contract. Until then, request-level actions stay read-only.",
                            "Войдите с session-backed user token, чтобы загрузить authoritative request-level governance contract. До этого request-level действия остаются read-only.",
                        )
                        .to_string()
                    }}
                </div>
            </Show>
            <div class="grid gap-3 lg:grid-cols-2">
                <Input
                    value=Signal::derive(move || governance_new_owner_user_id.get())
                    set_value=set_governance_new_owner_user_id
                    placeholder=tr(locale, "00000000-0000-0000-0000-000000000000", "00000000-0000-0000-0000-000000000000")
                    label=tr(locale, "New owner user id", "User id нового владельца")
                />
                <Input
                    value=Signal::derive(move || governance_reason_code.get())
                    set_value=set_governance_reason_code
                    placeholder=move || governance_reason_code_placeholder(
                        governance_intent_action.get().as_deref(),
                        &governance_actions_for_form.get(),
                        locale,
                    )
                    label=tr(locale, "Reason code", "Reason code")
                />
                <div class="flex flex-col gap-2">
                    <label class="text-sm font-medium leading-none">
                        {tr(locale, "Reason", "Причина")}
                    </label>
                    <textarea
                        class="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                        prop:value=move || governance_reason.get()
                        placeholder=move || governance_reason_placeholder(
                            governance_intent_action.get().as_deref(),
                            &governance_actions_for_form.get(),
                            locale,
                        )
                        on:input=move |event| {
                            set_governance_reason.set(event_target_value(&event));
                        }
                    ></textarea>
                    <p class="text-[11px] text-muted-foreground">
                        {move || governance_reason_placeholder(
                            governance_intent_action.get().as_deref(),
                            &governance_actions_for_form.get(),
                            locale,
                        )}
                    </p>
                </div>
            </div>
            <Show when=move || governance_intent_action.get().is_some()>
                <div class="rounded-md border border-border bg-background/80 px-3 py-2 text-xs text-muted-foreground">
                    {move || governance_action_requirement_hint(
                        governance_intent_action.get().as_deref(),
                        &governance_actions_for_form.get(),
                        locale,
                    ).unwrap_or_default()}
                </div>
            </Show>
            <div class="flex flex-wrap gap-2">
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "validate"))
                    on_click=Callback::new(move |_| on_validate.run(()))
                >
                    {tr(locale, "Validate", "Validate")}
                </Button>
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "approve"))
                    on_click=Callback::new(move |_| on_approve.run(()))
                >
                    {tr(locale, "Approve", "Approve")}
                </Button>
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "request_changes"))
                    on_click=Callback::new(move |_| on_request_changes.run(()))
                >
                    {tr(locale, "Request changes", "Запросить изменения")}
                </Button>
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "hold"))
                    on_click=Callback::new(move |_| on_hold.run(()))
                >
                    {tr(locale, "Hold", "Поставить на hold")}
                </Button>
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "resume"))
                    on_click=Callback::new(move |_| on_resume.run(()))
                >
                    {tr(locale, "Resume", "Возобновить")}
                </Button>
                <Button
                    class="h-8 px-3 py-1 text-xs"
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "reject"))
                    on_click=Callback::new(move |_| on_reject.run(()))
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
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "owner_transfer"))
                    on_click=Callback::new(move |_| on_transfer_owner.run(()))
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
                    disabled=Signal::derive(move || governance_submitting.get() || !governance_action_available(&governance_actions_for_form.get(), "yank"))
                    on_click=Callback::new(move |_| on_yank_release.run(()))
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
                    on_click=Callback::new(move |_| on_refresh.run(()))
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
                            on_click=Callback::new(move |_| {
                                match governance_confirmation_action.get().as_deref() {
                                    Some("reject") => on_reject.run(()),
                                    Some("owner-transfer") => on_transfer_owner.run(()),
                                    Some("yank") => on_yank_release.run(()),
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
    }
}
