use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::features::modules::transport::{finalize_module_transition, rollback_module_transition};
use rustok_api::{
    ModuleRetentionHoldView, ModuleTransitionCheckpointView, ModuleTransitionStateView,
};

fn short_digest(digest: &str) -> String {
    if digest.len() > 19 {
        format!("{}...{}", &digest[..10], &digest[digest.len() - 6..])
    } else {
        digest.to_string()
    }
}

#[component]
pub fn TransitionControlCard(
    checkpoint: ModuleTransitionCheckpointView,
    #[prop(default = vec![])] retention_holds: Vec<ModuleRetentionHoldView>,
    retention_holds_available: bool,
    #[prop(optional)] on_refresh: Option<Callback<()>>,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();

    let (is_busy, set_is_busy) = signal(false);
    let (action_error, set_action_error) = signal(Option::<String>::None);
    let (show_holds, set_show_holds) = signal(false);
    let (show_rollback_prompt, set_show_rollback_prompt) = signal(false);
    let (rollback_reason, set_rollback_reason) =
        signal("Rollback to retained predecessor after observation".to_string());

    let op_id = checkpoint.operation_id.clone();
    let is_observing = checkpoint.state == ModuleTransitionStateView::Observing;
    let is_past_point_of_no_return = checkpoint.state == ModuleTransitionStateView::PointOfNoReturn;
    let is_intervention_required =
        checkpoint.state == ModuleTransitionStateView::InterventionRequired;
    let is_converged = checkpoint.state == ModuleTransitionStateView::Converged;
    let recovery_limit_reached = checkpoint.recovery_attempt_count >= 1;
    let can_rollback = (is_observing || is_intervention_required)
        && checkpoint.predecessor_digest.is_some()
        && !recovery_limit_reached
        && !is_past_point_of_no_return
        && !is_converged;

    let state_badge_class = match checkpoint.state {
        ModuleTransitionStateView::Observing => {
            "bg-amber-500/15 text-amber-500 border-amber-500/30"
        }
        ModuleTransitionStateView::Converged => {
            "bg-emerald-500/15 text-emerald-500 border-emerald-500/30"
        }
        ModuleTransitionStateView::RecoveredToPredecessor => {
            "bg-indigo-500/15 text-indigo-500 border-indigo-500/30"
        }
        ModuleTransitionStateView::PointOfNoReturn => {
            "bg-purple-500/15 text-purple-500 border-purple-500/30"
        }
        ModuleTransitionStateView::InterventionRequired => {
            "bg-rose-500/15 text-rose-500 border-rose-500/30"
        }
        _ => "bg-blue-500/15 text-blue-500 border-blue-500/30",
    };

    let state_label = match checkpoint.state {
        ModuleTransitionStateView::Preflighting => "Preflighting",
        ModuleTransitionStateView::Fenced => "Fenced",
        ModuleTransitionStateView::Prestaging => "Pre-Staging",
        ModuleTransitionStateView::Activating => "Activating",
        ModuleTransitionStateView::Observing => "Observing Window",
        ModuleTransitionStateView::PointOfNoReturn => "Point of No Return (Irreversible)",
        ModuleTransitionStateView::RecoveredToPredecessor => "Recovered to Predecessor",
        ModuleTransitionStateView::Converged => "Converged",
        ModuleTransitionStateView::InterventionRequired => "Intervention Required",
    };

    let finalize_transition_action = Callback::new({
        let op_id = op_id.clone();
        let expected_revision = checkpoint.revision;
        move |()| {
            set_is_busy.set(true);
            set_action_error.set(None);

            let op_id = op_id.clone();
            let token_val = token.get();
            let tenant_val = tenant.get();
            let idempotency_key = uuid::Uuid::new_v4().to_string();

            leptos::task::spawn_local(async move {
                match finalize_module_transition(
                    token_val,
                    tenant_val,
                    op_id,
                    expected_revision,
                    idempotency_key,
                )
                .await
                {
                    Ok(_) => {
                        set_is_busy.set(false);
                        if let Some(cb) = on_refresh {
                            cb.run(());
                        }
                    }
                    Err(err) => {
                        set_is_busy.set(false);
                        set_action_error.set(Some(format!("Finalization failed: {err:?}")));
                    }
                }
            });
        }
    });

    let rollback_transition_action = Callback::new({
        let op_id = op_id.clone();
        move |()| {
            set_is_busy.set(true);
            set_action_error.set(None);

            let op_id = op_id.clone();
            let token_val = token.get();
            let tenant_val = tenant.get();
            let idempotency_key = uuid::Uuid::new_v4().to_string();
            let reason = rollback_reason.get();

            leptos::task::spawn_local(async move {
                match rollback_module_transition(
                    token_val,
                    tenant_val,
                    op_id,
                    reason,
                    idempotency_key,
                )
                .await
                {
                    Ok(_) => {
                        set_is_busy.set(false);
                        set_show_rollback_prompt.set(false);
                        if let Some(cb) = on_refresh {
                            cb.run(());
                        }
                    }
                    Err(err) => {
                        set_is_busy.set(false);
                        set_action_error.set(Some(format!("Rollback failed: {err:?}")));
                    }
                }
            });
        }
    });

    view! {
        <div class="rounded-xl border border-border bg-card p-6 shadow-sm transition-all">
            // Header
            <div class="flex flex-wrap items-center justify-between gap-4 border-b border-border pb-4">
                <div class="space-y-1">
                    <div class="flex items-center gap-2.5">
                        <h3 class="text-base font-semibold tracking-tight text-card-foreground">
                            "Module Transition: " {checkpoint.module_slug.clone()}
                        </h3>
                        <span class=format!("inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium {}", state_badge_class)>
                            {state_label}
                        </span>
                    </div>
                    <p class="text-xs text-muted-foreground">
                        "Operation ID: " <span class="font-mono">{checkpoint.operation_id.clone()}</span>
                    </p>
                </div>

                <div class="flex items-center gap-2">
                    <span class="inline-flex items-center rounded-md border border-border bg-secondary/50 px-2.5 py-1 text-xs font-medium text-secondary-foreground">
                        "Epoch #" {checkpoint.security_epoch}
                    </span>
                    <span class="inline-flex items-center rounded-md border border-border bg-secondary/50 px-2.5 py-1 text-xs font-medium text-secondary-foreground">
                        "Recovery Attempts: " {checkpoint.recovery_attempt_count} " / 1"
                    </span>
                </div>
            </div>

            // Digest Details & Status Details
            <div class="grid grid-cols-1 gap-4 py-4 md:grid-cols-2 text-xs">
                <div class="space-y-1.5 rounded-lg border border-border/50 bg-background/50 p-3">
                    <span class="font-medium text-muted-foreground">"Direct Predecessor (Hot-Standby N):"</span>
                    <div class="font-mono text-foreground break-all">
                        {checkpoint.predecessor_digest.as_deref().map(short_digest).unwrap_or_else(|| "None (Initial Install)".to_string())}
                    </div>
                </div>

                <div class="space-y-1.5 rounded-lg border border-border/50 bg-background/50 p-3">
                    <span class="font-medium text-muted-foreground">"Candidate Artifact (N+1):"</span>
                    <div class="font-mono text-foreground break-all">
                        {short_digest(&checkpoint.candidate_digest)}
                    </div>
                </div>
            </div>

            // Observation Details Notice
            {checkpoint.state_details.as_ref().map(|details| {
                view! {
                    <div class="mb-4 rounded-lg border border-blue-500/20 bg-blue-500/10 p-3 text-xs text-blue-400">
                        <span class="font-semibold">"Transition Detail: "</span> {details.clone()}
                    </div>
                }
            })}

            // Anti-Flapping Warning
            {if recovery_limit_reached && !is_converged {
                Some(view! {
                    <div class="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 text-xs text-amber-500">
                        <span class="font-semibold">"Zero-Flapping Invariant Enforced: "</span>
                        "Single automatic recovery attempt already executed. Automated bouncing disabled to protect persistent state."
                    </div>
                })
            } else {
                None
            }}

            // Point of No Return Notice
            {if is_past_point_of_no_return {
                Some(view! {
                    <div class="mb-4 rounded-lg border border-purple-500/20 bg-purple-500/10 p-3 text-xs text-purple-500">
                        <span class="font-semibold">"Point of No Return Reached: "</span>
                        "Destructive mutation, compensating action, or candidate-only settings active. Rollback window is closed and traffic/job/write fences are enforced."
                    </div>
                })
            } else {
                None
            }}

            // Intervention Required Notice
            {if is_intervention_required {
                Some(view! {
                    <div class="mb-4 rounded-lg border border-rose-500/20 bg-rose-500/10 p-3 text-xs text-rose-500">
                        <span class="font-semibold">"Intervention Required: "</span>
                        "Automatic transition action stopped, but serving selection was not changed. Explicitly roll back or deactivate the candidate if traffic must move."
                    </div>
                })
            } else {
                None
            }}

            // Error Display
            {move || action_error.get().map(|err| {
                view! {
                    <div class="mb-4 rounded-lg border border-rose-500/20 bg-rose-500/10 p-3 text-xs text-rose-500">
                        {err}
                    </div>
                }
            })}

            // Retention Holds Section
            {retention_holds_available.then(|| view! { <div class="border-t border-border pt-3">
                <button
                    type="button"
                    class="text-xs font-medium text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5"
                    on:click=move |_| set_show_holds.update(|v| *v = !*v)
                >
                    <span>{if show_holds.get() { "▼ Hide" } else { "▶ View" }}</span>
                    <span>"Tenant Retention Holds (" {retention_holds.len()} ")"</span>
                </button>

                {move || if show_holds.get() {
                    Some(view! {
                        <div class="mt-2.5 space-y-2 rounded-lg border border-border bg-background/40 p-3 text-xs">
                            {if retention_holds.is_empty() {
                                view! { <p class="text-muted-foreground">"No active GC retention holds."</p> }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-1.5">
                                        {retention_holds.iter().map(|hold| {
                                            view! {
                                                <div class="flex items-center justify-between border-b border-border/30 pb-1 font-mono text-[11px]">
                                                    <span class="text-foreground">{hold.target_type.clone()} ": " {short_digest(&hold.target_identity)}</span>
                                                    <span class="rounded bg-secondary/80 px-1.5 py-0.5 text-secondary-foreground">{hold.kind.clone()}</span>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                        </div>
                    })
                } else {
                    None
                }}
            </div> })}

            // Transition action
            <div class="mt-4 flex flex-col gap-3 border-t border-border pt-4">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div class="flex flex-wrap items-center gap-2">
                        // Finalize Transition Button
                        {if is_observing || is_past_point_of_no_return {
                            Some(view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-emerald-500 disabled:opacity-50"
                                    disabled=move || is_busy.get()
                                    on:click=move |_| finalize_transition_action.run(())
                                >
                                    {if is_busy.get() { "Finalizing..." } else { "Finalize Convergence" }}
                                </button>
                            })
                        } else {
                            None
                        }}

                        // Rollback Button
                        {if can_rollback {
                            Some(view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-rose-500 disabled:opacity-50"
                                    disabled=move || is_busy.get()
                                    on:click=move |_| set_show_rollback_prompt.update(|v| *v = !*v)
                                >
                                    {if show_rollback_prompt.get() { "Cancel Rollback" } else { "Rollback to Predecessor" }}
                                </button>
                            })
                        } else {
                            None
                        }}
                    </div>

                    <p class="text-[11px] text-muted-foreground">
                        {if can_rollback {
                            "Direct rollback will safely reactivate the retained predecessor, invalidate candidate bindings, and record recovery in the audit ledger."
                        } else if checkpoint.predecessor_digest.is_none() {
                            "Rollback is unavailable for initial installs (no retained predecessor)."
                        } else if recovery_limit_reached {
                            "Rollback limit reached (1 attempt maximum) to prevent flapping."
                        } else if is_past_point_of_no_return {
                            "Point of no return reached: rollback window closed."
                        } else {
                            "Transition reached terminal state."
                        }}
                    </p>
                </div>

                // Rollback Confirmation & Reason Form
                {move || if show_rollback_prompt.get() && can_rollback {
                    Some(view! {
                        <div class="rounded-lg border border-rose-500/30 bg-rose-500/10 p-3 space-y-2">
                            <div class="text-xs font-semibold text-rose-500">
                                "Confirm Rollback to Retained Predecessor"
                            </div>
                            <p class="text-[11px] text-muted-foreground">
                                "Please supply an operational reason for the audit ledger:"
                            </p>
                            <input
                                type="text"
                                class="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                                placeholder="Reason for rollback..."
                                prop:value=move || rollback_reason.get()
                                on:input=move |ev| set_rollback_reason.set(event_target_value(&ev))
                            />
                            <div class="flex items-center gap-2 pt-1">
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-rose-500 disabled:opacity-50"
                                    disabled=move || is_busy.get() || rollback_reason.get().trim().is_empty()
                                    on:click=move |_| rollback_transition_action.run(())
                                >
                                    {if is_busy.get() { "Rolling back..." } else { "Confirm & Execute Rollback" }}
                                </button>
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted"
                                    on:click=move |_| set_show_rollback_prompt.set(false)
                                >
                                    "Cancel"
                                </button>
                            </div>
                        </div>
                    })
                } else {
                    None
                }}
            </div>
        </div>
    }
}
