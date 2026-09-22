use crate::i18n::t;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn AuthAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    view! {
        <div class="space-y-6">
            <header class="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 shadow-sm">
                <div class="px-6 py-7 lg:px-8">
                    <div class="space-y-4">
                        <div class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/80 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.26em] text-muted-foreground">
                            <span class="h-2 w-2 rounded-full bg-indigo-500"></span>
                            {t_local("auth.badge", "Identity Foundation")}
                        </div>
                        <div class="space-y-2">
                            <h1 class="text-3xl font-semibold tracking-tight text-card-foreground">
                                {t_local("authAdmin.title", "Identity & Access Control Panel")}
                            </h1>
                            <p class="max-w-2xl text-sm leading-6 text-muted-foreground">
                                {t_local("authAdmin.subtitle", "Configure authentication policies, inspect active sessions, manage organization members, and register trusted OAuth client integrations.")}
                            </p>
                        </div>
                    </div>
                </div>
            </header>

            <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
                <a href="/users" class="group block rounded-2xl border bg-card p-5 shadow-sm transition-all hover:border-indigo-500/50 hover:shadow-md">
                    <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 transition-colors group-hover:bg-indigo-100">
                        <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
                        </svg>
                    </div>
                    <h3 class="mt-4 text-base font-semibold text-foreground group-hover:text-indigo-600">
                        {t_local("authAdmin.usersTitle", "User Accounts")}
                    </h3>
                    <p class="mt-2 text-xs leading-5 text-muted-foreground">
                        {t_local("authAdmin.usersDescription", "Manage member rosters, configure security roles, activate or suspend credentials, and inspect profiles.")}
                    </p>
                    <div class="mt-4 inline-flex items-center text-xs font-semibold text-indigo-600">
                        {t_local("authAdmin.usersAction", "Open Workspace ->")}
                    </div>
                </a>

                <a href="/apps" class="group block rounded-2xl border bg-card p-5 shadow-sm transition-all hover:border-indigo-500/50 hover:shadow-md">
                    <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 transition-colors group-hover:bg-indigo-100">
                        <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                        </svg>
                    </div>
                    <h3 class="mt-4 text-base font-semibold text-foreground group-hover:text-indigo-600">
                        {t_local("authAdmin.oauthTitle", "OAuth Connections")}
                    </h3>
                    <p class="mt-2 text-xs leading-5 text-muted-foreground">
                        {t_local("authAdmin.oauthDescription", "Register custom client integrations, manage callback endpoints, customize scopes, and rotate client secrets.")}
                    </p>
                    <div class="mt-4 inline-flex items-center text-xs font-semibold text-indigo-600">
                        {t_local("authAdmin.oauthAction", "Open Connections ->")}
                    </div>
                </a>

                <a href="/profile" class="group block rounded-2xl border bg-card p-5 shadow-sm transition-all hover:border-indigo-500/50 hover:shadow-md">
                    <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 transition-colors group-hover:bg-indigo-100">
                        <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                        </svg>
                    </div>
                    <h3 class="mt-4 text-base font-semibold text-foreground group-hover:text-indigo-600">
                        {t_local("authAdmin.profileTitle", "Profile Settings")}
                    </h3>
                    <p class="mt-2 text-xs leading-5 text-muted-foreground">
                        {t_local("authAdmin.profileDescription", "Configure your personal administrator profile, set name and details, and choose interface language preferences.")}
                    </p>
                    <div class="mt-4 inline-flex items-center text-xs font-semibold text-indigo-600">
                        {t_local("authAdmin.profileAction", "Open Profile ->")}
                    </div>
                </a>

                <a href="/security" class="group block rounded-2xl border bg-card p-5 shadow-sm transition-all hover:border-indigo-500/50 hover:shadow-md">
                    <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 transition-colors group-hover:bg-indigo-100">
                        <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                        </svg>
                    </div>
                    <h3 class="mt-4 text-base font-semibold text-foreground group-hover:text-indigo-600">
                        {t_local("authAdmin.securityTitle", "Security & Sessions")}
                    </h3>
                    <p class="mt-2 text-xs leading-5 text-muted-foreground">
                        {t_local("authAdmin.securityDescription", "Change password, review recent login activity logs, trace IP locations, and force logout active sessions.")}
                    </p>
                    <div class="mt-4 inline-flex items-center text-xs font-semibold text-indigo-600">
                        {t_local("authAdmin.securityAction", "Open Security ->")}
                    </div>
                </a>
            </div>
        </div>
    }
}
