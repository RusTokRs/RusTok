use crate::core::{
    CreateOAuthAppForm, OAuthAppListItemViewModel, oauth_app_list_item_view,
    oauth_app_type_defaults, prepare_create_oauth_app_input, prepare_update_oauth_app_input,
};
use crate::i18n::t;
use crate::model::{AppType, OAuthApp};
use crate::transport::{
    CreateOAuthAppResult, create_oauth_app, list_oauth_apps, revoke_oauth_app,
    rotate_oauth_app_secret, update_oauth_app,
};
use crate::ui::components::{Button, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{Badge, BadgeVariant};
use rustok_ui_core::UiRouteContext;

#[derive(Clone, PartialEq)]
enum ModalState {
    None,
    CreateApp,
    EditApp(OAuthApp),
    RotateSecret(OAuthApp),
    RevokeApp(OAuthApp),
    SecretRevealed { secret: String, app: OAuthApp },
}

#[component]
pub fn AppTypeBadge(app_type: AppType) -> impl IntoView {
    let (variant, label) = match app_type {
        AppType::Embedded => (BadgeVariant::Secondary, "Embedded"),
        AppType::FirstParty => (BadgeVariant::Default, "First Party"),
        AppType::Mobile => (BadgeVariant::Default, "Mobile"),
        AppType::Service => (BadgeVariant::Outline, "Service"),
        AppType::ThirdParty => (BadgeVariant::Warning, "Third Party"),
    };

    view! {
        <Badge variant=variant class="whitespace-nowrap">
            {label}
        </Badge>
    }
}

#[component]
pub fn OAuthAppsList(
    apps: Vec<OAuthApp>,
    loading: bool,
    on_edit_app: Callback<OAuthApp>,
    on_rotate_secret: Callback<OAuthApp>,
    on_revoke_app: Callback<OAuthApp>,
) -> impl IntoView {
    let rows_apps = apps.clone();
    let is_empty = apps.is_empty();

    view! {
        <div class="overflow-x-auto rounded-md border">
            <table class="w-full min-w-[960px] text-left text-sm">
                <thead class="bg-muted/50 text-xs uppercase text-muted-foreground">
                    <tr>
                        <th class="px-4 py-3 font-medium">"App"</th>
                        <th class="px-4 py-3 font-medium">"Type"</th>
                        <th class="px-4 py-3 font-medium">"Scopes / Grants"</th>
                        <th class="px-4 py-3 font-medium">"Client ID"</th>
                        <th class="px-4 py-3 font-medium">"Tokens"</th>
                        <th class="px-4 py-3 font-medium">"Last Used"</th>
                        <th class="px-4 py-3 text-right font-medium">"Actions"</th>
                    </tr>
                </thead>
                <tbody class="divide-y">
                    <Show when=move || loading>
                        <tr>
                            <td colspan="7" class="h-24 text-center text-muted-foreground">
                                "Loading app connections..."
                            </td>
                        </tr>
                    </Show>
                    {rows_apps
                        .into_iter()
                        .map(oauth_app_list_item_view)
                        .map(|item| {
                            let OAuthAppListItemViewModel {
                                app,
                                description,
                                scopes_summary,
                                grants_summary,
                                capability_label,
                                client_id,
                                last_used_at,
                            } = item;
                            let app_for_edit = app.clone();
                            let app_for_rotate = app.clone();
                            let app_for_revoke = app.clone();
                            let has_description = description.is_some();
                            let description = description.unwrap_or_default();

                            view! {
                                <tr class="transition-colors hover:bg-muted/40">
                                    <td class="px-4 py-3 align-top">
                                        <div class="font-medium text-slate-900">{app.name.clone()}</div>
                                        <div class="text-xs text-muted-foreground">{app.slug.clone()}</div>
                                        <Show when=move || has_description>
                                            <div class="mt-1 max-w-xs text-xs text-muted-foreground">
                                                {description.clone()}
                                            </div>
                                        </Show>
                                        <div class="mt-2 inline-flex rounded-full border px-2 py-1 text-xs text-muted-foreground">
                                            {capability_label}
                                        </div>
                                    </td>
                                    <td class="px-4 py-3 align-top">
                                        <AppTypeBadge app_type=app.app_type.clone() />
                                    </td>
                                    <td class="px-4 py-3 align-top text-xs text-slate-600">
                                        <div>
                                            <span class="font-medium text-slate-900">"Scopes: "</span>
                                            {scopes_summary.clone()}
                                        </div>
                                        <div class="mt-1">
                                            <span class="font-medium text-slate-900">"Grants: "</span>
                                            {grants_summary.clone()}
                                        </div>
                                    </td>
                                    <td class="px-4 py-3 align-top font-mono text-xs text-slate-500">
                                        {client_id}
                                    </td>
                                    <td class="px-4 py-3 align-top text-slate-500">
                                        {app.active_token_count}
                                    </td>
                                    <td class="px-4 py-3 align-top text-xs text-slate-500">
                                        {last_used_at}
                                    </td>
                                    <td class="px-4 py-3 align-top">
                                        <div class="flex justify-end gap-2">
                                            <Button
                                                class="h-8 bg-transparent px-3 py-1 text-xs text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                                                disabled=Signal::derive(move || !app.can_edit)
                                                on_click=Callback::new(move |_| on_edit_app.run(app_for_edit.clone()))
                                            >
                                                "Edit"
                                            </Button>
                                            <Button
                                                class="h-8 bg-transparent px-3 py-1 text-xs text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                                                disabled=Signal::derive(move || !app.can_rotate_secret)
                                                on_click=Callback::new(move |_| on_rotate_secret.run(app_for_rotate.clone()))
                                            >
                                                "Rotate Secret"
                                            </Button>
                                            <Button
                                                class="h-8 bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90"
                                                disabled=Signal::derive(move || !app.can_revoke)
                                                on_click=Callback::new(move |_| on_revoke_app.run(app_for_revoke.clone()))
                                            >
                                                "Revoke"
                                            </Button>
                                        </div>
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}

                    <Show when=move || !loading && is_empty>
                        <tr>
                            <td colspan="7" class="h-24 text-center text-muted-foreground">
                                "No app connections found."
                            </td>
                        </tr>
                    </Show>
                </tbody>
            </table>
        </div>
    }
}

#[component]
pub fn CreateAppForm(
    token: Option<String>,
    tenant: Option<String>,
    on_success: impl Fn(CreateOAuthAppResult) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (icon_url, set_icon_url) = signal(String::new());
    let (app_type, set_app_type) = signal("ThirdParty".to_string());
    let initial_defaults = oauth_app_type_defaults("ThirdParty");
    let (redirect_uris, set_redirect_uris) = signal(initial_defaults.redirect_uris.to_string());
    let (scopes, set_scopes) = signal(String::new());
    let (grant_types, set_grant_types) = signal(initial_defaults.grant_types.to_string());
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let submit = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let input = prepare_create_oauth_app_input(CreateOAuthAppForm {
            name: name.get_untracked(),
            slug: slug.get_untracked(),
            description: description.get_untracked(),
            icon_url: icon_url.get_untracked(),
            app_type: app_type.get_untracked(),
            redirect_uris: redirect_uris.get_untracked(),
            scopes: scopes.get_untracked(),
            grant_types: grant_types.get_untracked(),
        });
        let on_success = on_success.clone();

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result = create_oauth_app(Some(token_value), tenant_value, input).await;

            set_submitting.set(false);
            match result {
                Ok(res) => on_success(res),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Create New Connected App"</h3>

            <Input value=name set_value=set_name placeholder="My Integration" label="App Name" />
            <Input value=slug set_value=set_slug placeholder="com.example.app" label="Slug / Bundle ID" />
            <Input value=icon_url set_value=set_icon_url placeholder="https://example.com/icon.png" label="Icon URL" />

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Description"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    prop:value=description
                    on:input=move |ev| set_description.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"App Type"</label>
                <select
                    class="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    prop:value=app_type
                    on:change=move |ev| {
                        let next = event_target_value(&ev);
                        set_app_type.set(next.clone());
                        let defaults = oauth_app_type_defaults(&next);
                        set_redirect_uris.set(defaults.redirect_uris.to_string());
                        set_grant_types.set(defaults.grant_types.to_string());
                    }
                >
                    <option value="ThirdParty">"Third Party"</option>
                    <option value="Mobile">"Mobile"</option>
                    <option value="Service">"Service"</option>
                </select>
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Redirect URIs"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=redirect_uris
                    on:input=move |ev| set_redirect_uris.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Scopes"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=scopes
                    on:input=move |ev| set_scopes.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Grant Types"</label>
                <textarea
                    class="min-h-20 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=grant_types
                    on:input=move |ev| set_grant_types.set(event_target_value(&ev))
                />
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| submit())
                >
                    {move || if submitting.get() { "Creating..." } else { "Create App" }}
                </Button>
                <Button
                    class="bg-transparent text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                    on_click=Callback::new(move |_| on_cancel())
                >
                    "Cancel"
                </Button>
            </div>
        </div>
    }
}

#[component]
pub fn EditAppForm(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn(OAuthApp) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = signal(app.name.clone());
    let (description, set_description) = signal(app.description.clone().unwrap_or_default());
    let (icon_url, set_icon_url) = signal(app.icon_url.clone().unwrap_or_default());
    let (redirect_uris, set_redirect_uris) = signal(app.redirect_uris.join("\n"));
    let (scopes, set_scopes) = signal(app.scopes.join("\n"));
    let (grant_types, set_grant_types) = signal(app.grant_types.join("\n"));
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let submit = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let on_success = on_success.clone();
        let input = prepare_update_oauth_app_input(
            name.get_untracked(),
            description.get_untracked(),
            icon_url.get_untracked(),
            redirect_uris.get_untracked(),
            scopes.get_untracked(),
            grant_types.get_untracked(),
        );

        set_submitting.set(true);
        set_error.set(None);

        let app_id = app.id;
        spawn_local(async move {
            let result = update_oauth_app(Some(token_value), tenant_value, app_id, input).await;

            set_submitting.set(false);
            match result {
                Ok(response) => on_success(response),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Edit App Connection"</h3>

            <Input value=name set_value=set_name placeholder="My Integration" label="App Name" />
            <Input value=icon_url set_value=set_icon_url placeholder="https://example.com/icon.png" label="Icon URL" />

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Description"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    prop:value=description
                    on:input=move |ev| set_description.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Redirect URIs"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=redirect_uris
                    on:input=move |ev| set_redirect_uris.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Scopes"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=scopes
                    on:input=move |ev| set_scopes.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Grant Types"</label>
                <textarea
                    class="min-h-20 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=grant_types
                    on:input=move |ev| set_grant_types.set(event_target_value(&ev))
                />
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| submit())
                >
                    {move || if submitting.get() { "Saving..." } else { "Save Changes" }}
                </Button>
                <Button
                    class="bg-transparent text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                    on_click=Callback::new(move |_| on_cancel())
                >
                    "Cancel"
                </Button>
            </div>
        </div>
    }
}

#[component]
pub fn RotateSecretDialog(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn(String, OAuthApp) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let rotate = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let on_success = on_success.clone();
        let app_id = app.id;

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result = rotate_oauth_app_secret(Some(token_value), tenant_value, app_id).await;

            set_submitting.set(false);
            match result {
                Ok(response) => on_success(response.client_secret.clone(), response.app),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Rotate Client Secret"</h3>
            <p class="text-sm text-slate-500">
                "Rotate the secret for "<span class="font-semibold">{name}</span>"."
                <br/>
                "The old secret stops working immediately."
            </p>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| rotate())
                >
                    {move || if submitting.get() { "Rotating..." } else { "Rotate Secret" }}
                </Button>
                <Button
                    class="bg-transparent text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                    on_click=Callback::new(move |_| on_cancel())
                >
                    "Cancel"
                </Button>
            </div>
        </div>
    }
}

#[component]
pub fn RevokeAppDialog(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn() + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let revoke = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let on_success = on_success.clone();
        let app_id = app.id;

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result = revoke_oauth_app(Some(token_value), tenant_value, app_id).await;

            set_submitting.set(false);
            match result {
                Ok(_) => on_success(),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium text-red-600">"Revoke OAuth Application"</h3>
            <p class="text-sm text-slate-500">
                "Revoke access for "<span class="font-semibold">{name}</span>" and invalidate all active tokens."
            </p>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| revoke())
                >
                    {move || if submitting.get() { "Revoking..." } else { "Revoke Application" }}
                </Button>
                <Button
                    class="bg-transparent text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                    on_click=Callback::new(move |_| on_cancel())
                >
                    "Cancel"
                </Button>
            </div>
        </div>
    }
}

#[component]
pub fn OAuthAppsPage() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };
    let token = use_token();
    let tenant = use_tenant();

    let (apps, set_apps) = signal(Vec::<OAuthApp>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (modal_state, set_modal_state) = signal(ModalState::None);

    Effect::new(move |_| {
        let _ = refresh_counter.get();
        let token_value = token.get();
        let tenant_value = tenant.get();

        set_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            match list_oauth_apps(token_value, tenant_value).await {
                Ok(next_apps) => {
                    set_apps.set(next_apps);
                    set_loading.set(false);
                }
                Err(err) => {
                    set_error.set(Some(err.to_string()));
                    set_loading.set(false);
                }
            }
        });
    });

    let on_edit = Callback::new(move |app| set_modal_state.set(ModalState::EditApp(app)));
    let on_rotate = Callback::new(move |app| set_modal_state.set(ModalState::RotateSecret(app)));
    let on_revoke = Callback::new(move |app| set_modal_state.set(ModalState::RevokeApp(app)));

    let close_modal = move || set_modal_state.set(ModalState::None);

    view! {
        <div class="space-y-6">
            <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <h2 class="text-2xl font-bold tracking-tight">
                        {t_local("oauthApps.title", "OAuth App Connections")}
                    </h2>
                    <p class="text-muted-foreground">
                        {t_local("oauthApps.description", "Manage manual integrations, inspect manifest-managed frontends, and rotate client credentials.")}
                    </p>
                </div>
                <Button on_click=Callback::new(move |_| set_modal_state.set(ModalState::CreateApp))>
                    {t_local("oauthApps.create", "Create New App")}
                </Button>
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <OAuthAppsList
                apps=apps.get()
                loading=loading.get()
                on_edit_app=on_edit
                on_rotate_secret=on_rotate
                on_revoke_app=on_revoke
            />

            <Show when=move || modal_state.get() != ModalState::None>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-4 backdrop-blur-sm">
                    <div class="w-full max-w-2xl rounded-lg border bg-background p-6 shadow-lg">
                        {move || match modal_state.get() {
                            ModalState::CreateApp => {
                                let close = close_modal;
                                let token_value = token.get();
                                let tenant_value = tenant.get();
                                view! {
                                    <CreateAppForm
                                        token=token_value
                                        tenant=tenant_value
                                        on_success=move |result| {
                                            set_refresh_counter.update(|value| *value += 1);
                                            set_modal_state.set(ModalState::SecretRevealed {
                                                secret: result.client_secret,
                                                app: result.app,
                                            });
                                        }
                                        on_cancel=move || close()
                                    />
                                }
                                .into_any()
                            }
                            ModalState::EditApp(app) => {
                                let close = close_modal;
                                let token_value = token.get();
                                let tenant_value = tenant.get();
                                view! {
                                    <EditAppForm
                                        token=token_value
                                        tenant=tenant_value
                                        app=app
                                        on_success=move |_| {
                                            set_refresh_counter.update(|value| *value += 1);
                                            close();
                                        }
                                        on_cancel=move || close()
                                    />
                                }
                                .into_any()
                            }
                            ModalState::RotateSecret(app) => {
                                let close = close_modal;
                                let token_value = token.get();
                                let tenant_value = tenant.get();
                                view! {
                                    <RotateSecretDialog
                                        token=token_value
                                        tenant=tenant_value
                                        app=app
                                        on_success=move |new_secret, updated_app| {
                                            set_refresh_counter.update(|value| *value += 1);
                                            set_modal_state.set(ModalState::SecretRevealed {
                                                secret: new_secret,
                                                app: updated_app,
                                            });
                                        }
                                        on_cancel=move || close()
                                    />
                                }
                                .into_any()
                            }
                            ModalState::RevokeApp(app) => {
                                let close_for_success = close_modal;
                                let close_for_cancel = close_modal;
                                let token_value = token.get();
                                let tenant_value = tenant.get();
                                view! {
                                    <RevokeAppDialog
                                        token=token_value
                                        tenant=tenant_value
                                        app=app
                                        on_success=move || {
                                            set_refresh_counter.update(|value| *value += 1);
                                            close_for_success();
                                        }
                                        on_cancel=move || close_for_cancel()
                                    />
                                }
                                .into_any()
                            }
                            ModalState::SecretRevealed { secret, app } => {
                                let close = close_modal;
                                let title = if app.auto_created {
                                    t_local("oauthApps.secret.rotated", "Client secret rotated.")
                                } else {
                                    t_local("oauthApps.secret.generated", "Client secret generated.")
                                };

                                view! {
                                    <div class="space-y-4">
                                        <h3 class="text-lg font-medium text-green-600">{title}</h3>
                                        <p class="text-sm">
                                            {t_local("oauthApps.secret.warning", "Store this secret safely. It will not be shown again.")}
                                        </p>

                                        <div class="break-all rounded border bg-slate-100 p-3 font-mono text-sm">
                                            {secret}
                                        </div>

                                        <Button class="w-full" on_click=Callback::new(move |_| close())>
                                            {t_local("oauthApps.secret.saved", "I have saved it")}
                                        </Button>
                                    </div>
                                }
                                .into_any()
                            }
                            ModalState::None => view! { <div></div> }.into_any(),
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
