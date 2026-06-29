use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_hook_form::FormState;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use leptos_ui::{Select, SelectOption};
use rustok_api::UiRouteContext;

use crate::core::{graphql_user_view, prepare_update_user_input};
use crate::i18n::{auth_transport_error_message, t};
use crate::transport::{delete_user_details, fetch_user, update_user_details};
use crate::ui::components::{Button, Input, PageHeader};

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

#[derive(Params, PartialEq)]
struct UserParams {
    id: Option<String>,
}

#[component]
pub fn UserDetails() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = StoredValue::new(route_context.locale);
    let t_local =
        move |key: &str, fallback: &str| locale.with_value(|l| t(l.as_deref(), key, fallback));

    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();
    let params = use_params::<UserParams>();

    let user_resource = local_resource(
        move || params.with(|params| params.as_ref().ok().and_then(|params| params.id.clone())),
        move |_| {
            let token_value = token.get();
            let tenant_value = tenant.get();
            let user_id = params.with(|params| {
                params
                    .as_ref()
                    .ok()
                    .and_then(|params| params.id.clone())
                    .unwrap_or_default()
            });

            async move { fetch_user(user_id, token_value, tenant_value).await }
        },
    );

    let (is_editing, set_is_editing) = signal(false);
    let edit_name = signal(String::new());
    let edit_role = signal(String::new());
    let edit_status = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());

    let (show_delete_confirm, set_show_delete_confirm) = signal(false);
    let (delete_form_state, set_delete_form_state) = signal(FormState::idle());

    let navigate_back = navigate.clone();
    let go_back = Callback::new(move |_| {
        navigate_back("/users", Default::default());
    });

    let cancel_edit = Callback::new(move |_| {
        set_is_editing.set(false);
        set_form_state.set(FormState::idle());
    });

    let save_user = Callback::new(move |_| {
        let (name_signal, _) = edit_name;
        let (role_signal, _) = edit_role;
        let (status_signal, _) = edit_status;
        let user_id = params.with(|p| {
            p.as_ref()
                .ok()
                .and_then(|p| p.id.clone())
                .unwrap_or_default()
        });
        let name_val = name_signal.get();
        let role_val = role_signal.get();
        let status_val = status_signal.get();
        let token_val = token.get();
        let tenant_val = tenant.get();

        set_form_state.set(FormState::submitting());

        spawn_local(async move {
            let input = prepare_update_user_input(name_val, role_val, status_val);
            match update_user_details(token_val, tenant_val, user_id, input).await {
                Ok(_) => {
                    set_form_state.set(FormState::idle());
                    set_is_editing.set(false);
                    user_resource.refetch();
                }
                Err(e) => {
                    set_form_state.set(FormState::with_form_error(locale.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &e.to_string())
                    })));
                }
            }
        });
    });

    let confirm_delete = {
        let navigate = navigate.clone();
        Callback::new(move |_| {
            let user_id = params.with(|p| {
                p.as_ref()
                    .ok()
                    .and_then(|p| p.id.clone())
                    .unwrap_or_default()
            });
            let token_val = token.get();
            let tenant_val = tenant.get();

            set_delete_form_state.set(FormState::submitting());

            let navigate_to_users = navigate.clone();
            spawn_local(async move {
                match delete_user_details(token_val, tenant_val, user_id).await {
                    Ok(_) => {
                        navigate_to_users("/users", Default::default());
                    }
                    Err(e) => {
                        set_delete_form_state.set(FormState::with_form_error(locale.with_value(
                            |locale| {
                                auth_transport_error_message(locale.as_deref(), &e.to_string())
                            },
                        )));
                        set_show_delete_confirm.set(false);
                    }
                }
            });
        })
    };

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_local("users.detail.title", "User profile")
                subtitle=t_local("users.detail.subtitle", "User details from GraphQL.")
                eyebrow=t_local("app.nav.users", "Users")
                actions=view! {
                    <Button
                        on_click=go_back
                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {t_local("users.detail.back", "Back to users")}
                    </Button>
                    <Show when=move || !is_editing.get()>
                        <Button
                            on_click=move |_| {
                                if let Some(Ok(ref resp)) = user_resource.get() {
                                    if let Some(ref user) = resp.user {
                                        let user = graphql_user_view(user.clone(), String::new());
                                        let (_, set_n) = edit_name;
                                        let (_, set_r) = edit_role;
                                        let (_, set_s) = edit_status;
                                        set_n.set(user.edit_form.name);
                                        set_r.set(user.edit_form.role);
                                        set_s.set(user.edit_form.status);
                                        set_form_state.set(FormState::idle());
                                        set_is_editing.set(true);
                                    }
                                }
                            }
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {t_local("users.detail.edit", "Edit")}
                        </Button>
                        <Button
                            on_click=move |_| set_show_delete_confirm.set(true)
                            class="border border-destructive/30 bg-transparent text-destructive hover:bg-destructive/10"
                        >
                            {t_local("users.detail.delete", "Delete")}
                        </Button>
                    </Show>
                    <Show when=move || is_editing.get()>
                        <Button
                            on_click=save_user
                            disabled=Signal::derive(move || form_state.get().is_submitting)
                        >
                            {move || if form_state.get().is_submitting {
                                t_local("users.detail.saving", "Saving...")
                            } else {
                                t_local("users.detail.save", "Save")
                            }}
                        </Button>
                        <Button
                            on_click=cancel_edit
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {t_local("users.detail.cancel", "Cancel")}
                        </Button>
                    </Show>
                }
                .into_any()
            />

            <Show when=move || form_state.get().form_error.is_some()>
                <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                    {move || form_state.get().form_error.unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || show_delete_confirm.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
                    <div class="w-full max-w-sm rounded-xl border border-border bg-card p-6 shadow-xl">
                        <h3 class="mb-2 text-lg font-semibold text-card-foreground">
                            {t_local("users.detail.deleteConfirmTitle", "Delete user?")}
                        </h3>
                        <p class="mb-4 text-sm text-muted-foreground">
                            {t_local("users.detail.deleteConfirmText", "This action cannot be undone. The user will be permanently removed.")}
                        </p>
                        <Show when=move || delete_form_state.get().form_error.is_some()>
                            <div class="mb-3 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {move || delete_form_state.get().form_error.unwrap_or_default()}
                            </div>
                        </Show>
                        <div class="flex gap-3">
                            <Button
                                on_click=confirm_delete.clone()
                                disabled=Signal::derive(move || delete_form_state.get().is_submitting)
                                class="flex-1 bg-destructive text-destructive-foreground hover:bg-destructive/90"
                            >
                                {move || if delete_form_state.get().is_submitting {
                                    t_local("users.detail.deleting", "Deleting...")
                                } else {
                                    t_local("users.detail.confirmDelete", "Delete")
                                }}
                            </Button>
                            <Button
                                on_click=move |_| set_show_delete_confirm.set(false)
                                class="flex-1 border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                disabled=Signal::derive(move || delete_form_state.get().is_submitting)
                            >
                                {t_local("users.detail.cancel", "Cancel")}
                            </Button>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {t_local("users.detail.section", "Profile")}
                </h4>
                <Suspense
                    fallback=move || view! {
                        <p class="text-sm text-muted-foreground">
                            {t_local("users.detail.loading", "Loading...")}
                        </p>
                    }
                >
                    {move || match user_resource.get() {
                        None => view! {
                            <p class="text-sm text-muted-foreground">
                                {t_local("users.detail.pending", "Waiting for response...")}
                            </p>
                        }
                        .into_any(),
                        Some(Ok(response)) => {
                            if let Some(user) = response.user {
                                let user = graphql_user_view(
                                    user,
                                    t_local("users.placeholderDash", "—"),
                                );
                                let email = user.email;
                                let name_display = user.name;
                                let role_display = user.role;
                                let status_display = user.status;
                                let tenant_display = user.tenant_name;
                                let created_at = user.created_at;
                                let id = user.id;

                                view! {
                                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.email", "Email")}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{email}</p>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.name", "Name")}
                                            </span>
                                            <Show
                                                when=move || is_editing.get()
                                                fallback={
                                                    let v = name_display.clone();
                                                    move || view! { <p class="mt-1 text-sm text-foreground">{v.clone()}</p> }
                                                }
                                            >
                                                <div class="mt-1">
                                                    <Input
                                                        value=edit_name.0
                                                        set_value=edit_name.1
                                                        placeholder="Full name"
                                                        label=move || String::new()
                                                    />
                                                </div>
                                            </Show>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.role", "Role")}
                                            </span>
                                            <Show
                                                when=move || is_editing.get()
                                                fallback={
                                                    let v = role_display.clone();
                                                    move || view! { <p class="mt-1 text-sm text-foreground">{v.clone()}</p> }
                                                }
                                            >
                                                <div class="mt-1">
                                                    <Select
                                                        options=vec![
                                                            SelectOption::new("CUSTOMER", "Customer"),
                                                            SelectOption::new("MANAGER", "Manager"),
                                                            SelectOption::new("ADMIN", "Admin"),
                                                            SelectOption::new("SUPER_ADMIN", "Super Admin"),
                                                        ]
                                                        value=edit_role.0
                                                        set_value=edit_role.1
                                                    />
                                                </div>
                                            </Show>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.status", "Status")}
                                            </span>
                                            <Show
                                                when=move || is_editing.get()
                                                fallback={
                                                    let v = status_display.clone();
                                                    move || view! { <p class="mt-1 text-sm text-foreground">{v.clone()}</p> }
                                                }
                                            >
                                                <div class="mt-1">
                                                    <Select
                                                        options=vec![
                                                            SelectOption::new("ACTIVE", "Active"),
                                                            SelectOption::new("INACTIVE", "Inactive"),
                                                            SelectOption::new("BANNED", "Banned"),
                                                        ]
                                                        value=edit_status.0
                                                        set_value=edit_status.1
                                                    />
                                                </div>
                                            </Show>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                "Tenant"
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{tenant_display}</p>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.createdAt", "Created")}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{created_at}</p>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {t_local("users.detail.id", "User ID")}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{id}</p>
                                        </div>
                                    </div>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                        {t_local("users.detail.empty", "User not found.")}
                                    </div>
                                }
                                .into_any()
                            }
                        }
                        Some(Err(_err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {t_local("users.detail.loadError", "Failed to load this user. Check API availability and access permissions.")}
                            </div>
                        }
                        .into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
