use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_auth, use_tenant, use_token};
use rustok_ui_core::UiRouteContext;
use rustok_ui_forms::FormState;

use crate::core::{ChangePasswordInputError, prepare_change_password_request};
use crate::i18n::{auth_transport_error_message, t};
use crate::transport::change_password;
use crate::ui::components::{Button, Input, PageHeader};

/// Do not sign out a newer identity when the revoke-other-sessions request completes late.
struct SessionRevocationFence {
    scope: leptos_auth::ClientAuthScope,
    token: String,
    tenant: String,
}

impl SessionRevocationFence {
    fn capture(auth: &leptos_auth::AuthContext) -> Option<Self> {
        let session = auth.session().try_get_untracked().flatten()?;
        let fence = Self {
            scope: auth.client_scope_untracked(),
            token: session.token,
            tenant: session.tenant,
        };
        fence.is_current(auth).then_some(fence)
    }

    fn is_current(&self, auth: &leptos_auth::AuthContext) -> bool {
        self.scope.is_ready()
            && auth.client_scope_untracked() == self.scope
            && !auth.is_token_expired()
            && auth
                .session()
                .try_get_untracked()
                .flatten()
                .is_some_and(|session| session.token == self.token && session.tenant == self.tenant)
            && leptos_auth::storage::get_token().as_deref() == Some(self.token.as_str())
            && leptos_auth::storage::matches_request_tenant(&self.tenant)
    }
}

#[component]
pub fn Security() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    let auth = use_auth();
    let token = use_token();
    let tenant = use_tenant();

    let (current_password, set_current_password) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (success_message, set_success_message) = signal(Option::<String>::None);
    let (session_action_error, set_session_action_error) = signal(Option::<String>::None);
    let (session_action_busy, set_session_action_busy) = signal(false);
    let auth_loading = auth.is_loading();

    let on_change_password = Callback::new(move |_| {
        let request = match prepare_change_password_request(
            token.get(),
            tenant.get(),
            current_password.get(),
            new_password.get(),
        ) {
            Ok(request) => request,
            Err(ChangePasswordInputError::MissingPasswords) => {
                set_form_state.set(FormState::with_form_error(t_local(
                    "security.passwordRequired",
                    "Enter current and new passwords.",
                )));
                return;
            }
            Err(ChangePasswordInputError::Unauthorized) => {
                set_form_state.set(FormState::with_form_error(t_local(
                    "errors.auth.unauthorized",
                    "You are not authorized to perform this action.",
                )));
                return;
            }
            Err(ChangePasswordInputError::MissingTenant) => {
                set_form_state.set(FormState::with_form_error(t_local(
                    "errors.auth.missingTenant",
                    "Tenant context is required.",
                )));
                return;
            }
        };

        set_form_state.set(FormState::submitting());
        set_success_message.set(None);

        spawn_local(async move {
            let result = change_password(
                request.token,
                request.tenant,
                request.current_password,
                request.new_password,
            )
            .await;

            match result {
                Ok(_) => {
                    set_form_state.set(FormState::idle());
                    set_success_message.set(Some(t_local(
                        "security.passwordUpdated",
                        "Password updated successfully.",
                    )));
                    set_current_password.set(String::new());
                    set_new_password.set(String::new());
                }
                Err(err_str) => {
                    let message = locale_stored.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &err_str)
                    });
                    set_form_state.set(FormState::with_form_error(message));
                    set_success_message.set(None);
                }
            }
        });
    });

    let (refresh_trigger, set_refresh_trigger) = signal(0usize);
    // Resolved by the request: the page mounts inside the `Async` admin layout, so a
    // `LocalResource` here leaves the reader with the `Suspense` fallback.
    let sessions = Resource::new(
        move || (token.get(), tenant.get(), refresh_trigger.get()),
        move |(tok, ten, _generation)| async move {
            crate::transport::list_sessions(tok, ten, Some(50)).await
        },
    );

    let on_revoke_session = Callback::new(move |session_id: String| {
        let tok = token.get();
        let ten = tenant.get();
        spawn_local(async move {
            if let Ok(true) = crate::transport::revoke_session(tok, ten, session_id).await {
                set_refresh_trigger.update(|n| *n += 1);
            }
        });
    });

    let on_sign_out_all = Callback::new(move |_| {
        if auth.is_loading().try_get_untracked().unwrap_or(true)
            || session_action_busy.try_get_untracked().unwrap_or(true)
        {
            return;
        }
        let failure = t_local(
            "security.signOutAllFailed",
            "Sign-out could not be confirmed. Check active sessions or retry from the account menu.",
        );
        let Some(fence) = SessionRevocationFence::capture(&auth) else {
            let _ = set_session_action_error.try_set(Some(failure));
            return;
        };
        if set_session_action_busy.try_update(|busy| {
            if *busy {
                false
            } else {
                *busy = true;
                true
            }
        }) != Some(true)
        {
            return;
        }
        let _ = set_session_action_error.try_set(None);
        let auth = auth.clone();
        spawn_local(async move {
            // The scheduled task can start after a route or auth transition.
            let confirmed = if fence.is_current(&auth) {
                crate::transport::revoke_all_sessions(
                    Some(fence.token.clone()),
                    Some(fence.tenant.clone()),
                )
                .await
                .is_ok()
            } else {
                false
            };
            let signed_out = if confirmed && fence.is_current(&auth) {
                auth.sign_out().await.is_ok()
            } else {
                false
            };
            if !signed_out {
                let _ = set_session_action_error.try_set(Some(failure));
            }
            let _ = set_session_action_busy.try_set(false);
        });
    });

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_local("security.title", "Security & sessions")
                subtitle=t_local("security.subtitle", "Monitor active sessions and keep credentials secure.")
                eyebrow=t_local("security.badge", "Security")
                actions=view! {
                    <Button
                        on_click=on_sign_out_all
                        disabled=Signal::derive(move || session_action_busy.get() || auth_loading.get())
                        class="border border-border bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {t_local("security.signOutAll", "Sign out all sessions")}
                    </Button>
                }
                .into_any()
            />
            <Show when=move || session_action_error.get().is_some()>
                <p role="alert" class="mb-4 rounded-md border border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive">
                    {move || session_action_error.get().unwrap_or_default()}
                </p>
            </Show>

            <div class="grid gap-6 lg:grid-cols-2">
                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {t_local("security.passwordTitle", "Change password")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {t_local("security.passwordSubtitle", "Use a strong password unique to this admin account.")}
                    </p>
                     <Input
                        value=current_password
                        set_value=set_current_password
                        placeholder="••••••••"
                        type_="password"
                        label=t_local("security.currentPasswordLabel", "Current password")
                    />
                    <Input
                        value=new_password
                        set_value=set_new_password
                        placeholder="••••••••"
                        type_="password"
                        label=t_local("security.newPasswordLabel", "New password")
                    />
                    <p class="text-sm text-muted-foreground">
                        {t_local("security.passwordHint", "Use 12+ characters with mixed case and symbols.")}
                    </p>
                    <Button on_click=on_change_password class="w-full">
                        {t_local("security.passwordSubmit", "Update password")}
                    </Button>
                    <Show when=move || form_state.get().form_error.is_some()>
                        <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || form_state.get().form_error.unwrap_or_default()}
                        </div>
                    </Show>
                    <Show when=move || success_message.get().is_some()>
                        <div class="rounded-md bg-emerald-100 border border-emerald-200 px-4 py-2 text-sm text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
                            {move || success_message.get().unwrap_or_default()}
                        </div>
                    </Show>
                </div>

                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">
                                {t_local("security.sessionsTitle", "Active sessions")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {t_local("security.sessionsSubtitle", "Review devices that are currently signed in.")}
                            </p>
                        </div>
                        <button
                            type="button"
                            on:click=move |_| set_refresh_trigger.update(|n| *n += 1)
                            class="text-xs px-2.5 py-1 rounded border border-border bg-background hover:bg-muted text-foreground transition-colors cursor-pointer"
                        >
                            "Refresh"
                        </button>
                    </div>

                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-lg bg-muted" /> }>
                        {let view_sessions = sessions;
                        move || {
                            view_sessions.get().map(|res| match res {
                                Ok(items) if items.is_empty() => view! {
                                    <div class="rounded-lg bg-muted px-4 py-8 text-center text-sm text-muted-foreground">
                                        "No active sessions found."
                                    </div>
                                }.into_any(),
                                Ok(items) => view! {
                                    <div class="divide-y divide-border/40 overflow-hidden rounded-lg border border-border">
                                        {items.into_iter().map(|s| {
                                            let s_id = s.id.clone();
                                            let is_curr = s.current;
                                            let ua = s.user_agent.unwrap_or_else(|| "Unknown device / agent".to_string());
                                            let ip = s.ip_address.unwrap_or_else(|| "Unknown IP".to_string());
                                            let on_revoke = on_revoke_session;
                                            view! {
                                                <div class="flex items-center justify-between p-3 bg-background hover:bg-muted/30 transition-colors">
                                                    <div class="space-y-1 min-w-0 pr-3">
                                                        <div class="flex items-center gap-2">
                                                            <span class="text-xs font-semibold truncate text-foreground">{ua}</span>
                                                            {if is_curr {
                                                                view! {
                                                                    <span class="rounded-full bg-emerald-100 text-emerald-800 dark:bg-emerald-950/60 dark:text-emerald-300 px-2 py-0.5 text-[10px] font-medium">
                                                                        "Current"
                                                                    </span>
                                                                }.into_any()
                                                            } else {
                                                                ().into_any()
                                                            }}
                                                        </div>
                                                        <div class="flex items-center gap-2 text-[11px] text-muted-foreground font-mono">
                                                            <span>{ip}</span>
                                                            <span>"•"</span>
                                                            <span>{s.created_at}</span>
                                                        </div>
                                                    </div>
                                                    <div>
                                                        {if !is_curr {
                                                            let id_clone = s_id.clone();
                                                            view! {
                                                                <button
                                                                    type="button"
                                                                    class="text-xs text-destructive hover:underline cursor-pointer font-medium px-2 py-1"
                                                                    on:click=move |_| on_revoke.run(id_clone.clone())
                                                                >
                                                                    "Revoke"
                                                                </button>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <span class="text-xs text-muted-foreground italic px-2">"Active"</span>
                                                            }.into_any()
                                                        }}
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-lg bg-destructive/10 border border-destructive/20 p-4 text-xs text-destructive">
                                        {format!("Failed to load sessions: {err}")}
                                    </div>
                                }.into_any(),
                            })
                        }}
                    </Suspense>
                </div>
            </div>
        </section>
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod session_revocation_tests {
    use super::*;

    #[test]
    fn revocation_fence_requires_a_live_current_owner_and_session() {
        let owner = Owner::new();
        let (auth, fence) = owner.with(|| {
            let session = serde_json::from_value(serde_json::json!({
                "token": "test-access",
                "expires_at": i64::MAX,
                "tenant": "shop"
            }))
            .unwrap();
            leptos_auth::provide_server_auth_snapshot(leptos_auth::ServerAuthSnapshot {
                session: Some(session),
                user: None,
            });
            let auth = leptos_auth::AuthContext::new();
            let fence = SessionRevocationFence::capture(&auth).unwrap();
            assert!(fence.is_current(&auth));
            (auth, fence)
        });
        owner.cleanup();
        assert!(!fence.is_current(&auth));
        assert!(SessionRevocationFence::capture(&auth).is_none());

        let expired_owner = Owner::new();
        expired_owner.with(|| {
            let session = serde_json::from_value(serde_json::json!({
                "token": "expired-access",
                "expires_at": 0,
                "tenant": "shop"
            }))
            .unwrap();
            leptos_auth::provide_server_auth_snapshot(leptos_auth::ServerAuthSnapshot {
                session: Some(session),
                user: None,
            });
            let auth = leptos_auth::AuthContext::new();
            assert!(SessionRevocationFence::capture(&auth).is_none());
        });
    }
}
