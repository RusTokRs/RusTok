use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::shared::api::queries::{USER_DETAILS_QUERY, USER_DETAILS_QUERY_HASH};
use crate::shared::api::{request, request_with_persisted, ApiError};
use crate::shared::ui::{Button, Input, LanguageToggle};
use crate::{t_string, use_i18n};
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_hook_form::FormState;
use leptos_ui::{Select, SelectOption};

#[derive(Params, PartialEq)]
struct UserParams {
    id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlUserResponse {
    user: Option<GraphqlUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlUser {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
    status: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "tenantName")]
    tenant_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct UserVariables {
    id: String,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

async fn fetch_user_graphql(
    user_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUserResponse, ApiError> {
    request_with_persisted::<UserVariables, GraphqlUserResponse>(
        USER_DETAILS_QUERY,
        UserVariables { id: user_id },
        USER_DETAILS_QUERY_HASH,
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_user_server(user_id: String) -> Result<GraphqlUserResponse, ServerFnError> {
    user_details_native(user_id).await
}

async fn fetch_user(
    user_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUserResponse, String> {
    match fetch_user_server(user_id.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_user_graphql(user_id, token, tenant_slug)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/user-details")]
async fn user_details_native(id: String) -> Result<GraphqlUserResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_READ) {
            return Err(ServerFnError::new("users:read required"));
        }

        let user_id =
            Uuid::parse_str(&id).map_err(|err| server_error(format!("invalid user id: {err}")))?;
        let app_ctx = expect_context::<AppContext>();

        let statement = match app_ctx.db.get_database_backend() {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    u.id,
                    u.email,
                    u.name,
                    COALESCE((
                        SELECT r.slug
                        FROM user_roles ur
                        JOIN roles r ON r.id = ur.role_id
                        WHERE ur.user_id = u.id
                          AND r.tenant_id = u.tenant_id
                        ORDER BY CASE r.slug
                            WHEN 'super_admin' THEN 1
                            WHEN 'admin' THEN 2
                            WHEN 'manager' THEN 3
                            WHEN 'customer' THEN 4
                            ELSE 5
                        END
                        LIMIT 1
                    ), 'customer') AS role,
                    u.status,
                    u.created_at,
                    t.name AS tenant_name
                FROM users u
                JOIN tenants t ON t.id = u.tenant_id
                WHERE u.id = ?1
                  AND u.tenant_id = ?2
                "#,
                vec![user_id.into(), tenant.id.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    u.id,
                    u.email,
                    u.name,
                    COALESCE((
                        SELECT r.slug
                        FROM user_roles ur
                        JOIN roles r ON r.id = ur.role_id
                        WHERE ur.user_id = u.id
                          AND r.tenant_id = u.tenant_id
                        ORDER BY CASE r.slug
                            WHEN 'super_admin' THEN 1
                            WHEN 'admin' THEN 2
                            WHEN 'manager' THEN 3
                            WHEN 'customer' THEN 4
                            ELSE 5
                        END
                        LIMIT 1
                    ), 'customer') AS role,
                    u.status,
                    u.created_at,
                    t.name AS tenant_name
                FROM users u
                JOIN tenants t ON t.id = u.tenant_id
                WHERE u.id = $1
                  AND u.tenant_id = $2
                "#,
                vec![user_id.into(), tenant.id.into()],
            ),
        };

        let user = match app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
        {
            Some(row) => Some(GraphqlUser {
                id: row
                    .try_get::<Uuid>("", "id")
                    .map(|value| value.to_string())
                    .map_err(|err| server_error(err.to_string()))?,
                email: row
                    .try_get("", "email")
                    .map_err(|err| server_error(err.to_string()))?,
                name: row
                    .try_get("", "name")
                    .map_err(|err| server_error(err.to_string()))?,
                role: row
                    .try_get("", "role")
                    .map_err(|err| server_error(err.to_string()))?,
                status: row
                    .try_get::<rustok_core::UserStatus>("", "status")
                    .map(|value| value.to_string())
                    .map_err(|err| server_error(err.to_string()))?,
                created_at: row
                    .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
                    .map(|value| value.to_rfc3339())
                    .map_err(|err| server_error(err.to_string()))?,
                tenant_name: row
                    .try_get("", "tenant_name")
                    .map_err(|err| server_error(err.to_string()))?,
            }),
            None => None,
        };

        Ok(GraphqlUserResponse { user })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/user-details requires the `ssr` feature",
        ))
    }
}

#[derive(Clone, Debug, Serialize)]
struct UpdateUserVariables {
    id: String,
    input: UpdateUserInput,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateUserInput {
    name: Option<String>,
    role: String,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateUserResponse {
    #[serde(rename = "updateUser")]
    #[allow(dead_code)]
    update_user: Option<GraphqlUser>,
}

#[derive(Clone, Debug, Serialize)]
struct DeleteUserVariables {
    id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct DeleteUserResponse {
    #[serde(rename = "deleteUser")]
    #[allow(dead_code)]
    delete_user: Option<DeleteResult>,
}

#[derive(Clone, Debug, Deserialize)]
struct DeleteResult {
    #[allow(dead_code)]
    success: bool,
}

const UPDATE_USER_MUTATION: &str = r#"
mutation UpdateUser($id: UUID!, $input: UpdateUserInput!) {
    updateUser(id: $id, input: $input) {
        id email name role status createdAt tenantName
    }
}
"#;

const DELETE_USER_MUTATION: &str = r#"
mutation DeleteUser($id: UUID!) {
    deleteUser(id: $id) {
        success
    }
}
"#;

#[component]
pub fn UserDetails() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();
    let params = use_params::<UserParams>();

    let user_resource = Resource::new(
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
    let go_back = move |_| {
        navigate_back("/users", Default::default());
    };

    let cancel_edit = move |_| {
        set_is_editing.set(false);
        set_form_state.set(FormState::idle());
    };

    let save_user = move |_| {
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
            let vars = UpdateUserVariables {
                id: user_id,
                input: UpdateUserInput {
                    name: if name_val.is_empty() {
                        None
                    } else {
                        Some(name_val)
                    },
                    role: role_val,
                    status: status_val,
                },
            };
            match request::<UpdateUserVariables, UpdateUserResponse>(
                UPDATE_USER_MUTATION,
                vars,
                token_val,
                tenant_val,
            )
            .await
            {
                Ok(_) => {
                    set_form_state.set(FormState::idle());
                    set_is_editing.set(false);
                    user_resource.refetch();
                }
                Err(e) => {
                    set_form_state.set(FormState::with_form_error(format!("{:?}", e)));
                }
            }
        });
    };

    let confirm_delete = {
        let navigate = navigate.clone();
        move |_| {
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
                let vars = DeleteUserVariables { id: user_id };
                match request::<DeleteUserVariables, DeleteUserResponse>(
                    DELETE_USER_MUTATION,
                    vars,
                    token_val,
                    tenant_val,
                )
                .await
                {
                    Ok(_) => {
                        navigate_to_users("/users", Default::default());
                    }
                    Err(e) => {
                        set_delete_form_state.set(FormState::with_form_error(format!("{:?}", e)));
                        set_show_delete_confirm.set(false);
                    }
                }
            });
        }
    };

    view! {
        <section class="px-10 py-8">
            <header class="mb-8 flex flex-wrap items-center justify-between gap-4">
                <div>
                    <span class="inline-flex items-center rounded-full bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                        {move || t_string!(i18n, app.nav.users)}
                    </span>
                    <h1 class="mt-2 text-2xl font-semibold text-foreground">
                        {move || t_string!(i18n, users.detail.title)}
                    </h1>
                    <p class="mt-2 text-sm text-muted-foreground">
                        {move || t_string!(i18n, users.detail.subtitle)}
                    </p>
                </div>
                <div class="flex flex-wrap items-center gap-3">
                    <LanguageToggle />
                    <Button
                        on_click=go_back
                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {move || t_string!(i18n, users.detail.back)}
                    </Button>
                    <Show when=move || !is_editing.get()>
                        <Button
                            on_click=move |_| {
                                if let Some(Ok(ref resp)) = user_resource.get() {
                                    if let Some(ref user) = resp.user {
                                        let (_, set_n) = edit_name;
                                        let (_, set_r) = edit_role;
                                        let (_, set_s) = edit_status;
                                        set_n.set(user.name.clone().unwrap_or_default());
                                        set_r.set(user.role.clone());
                                        set_s.set(user.status.clone());
                                        set_form_state.set(FormState::idle());
                                        set_is_editing.set(true);
                                    }
                                }
                            }
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {move || t_string!(i18n, users.detail.edit)}
                        </Button>
                        <Button
                            on_click=move |_| set_show_delete_confirm.set(true)
                            class="border border-destructive/30 bg-transparent text-destructive hover:bg-destructive/10"
                        >
                            {move || t_string!(i18n, users.detail.delete)}
                        </Button>
                    </Show>
                    <Show when=move || is_editing.get()>
                        <Button
                            on_click=save_user
                            disabled=Signal::derive(move || form_state.get().is_submitting)
                        >
                            {move || if form_state.get().is_submitting {
                                t_string!(i18n, users.detail.saving).to_string()
                            } else {
                                t_string!(i18n, users.detail.save).to_string()
                            }}
                        </Button>
                        <Button
                            on_click=cancel_edit
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {move || t_string!(i18n, users.detail.cancel)}
                        </Button>
                    </Show>
                </div>
            </header>

            <Show when=move || form_state.get().form_error.is_some()>
                <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                    {move || form_state.get().form_error.unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || show_delete_confirm.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
                    <div class="w-full max-w-sm rounded-2xl bg-card p-6 shadow-xl border border-border">
                        <h3 class="mb-2 text-lg font-semibold text-card-foreground">
                            {move || t_string!(i18n, users.detail.deleteConfirmTitle)}
                        </h3>
                        <p class="mb-4 text-sm text-muted-foreground">
                            {move || t_string!(i18n, users.detail.deleteConfirmText)}
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
                                    t_string!(i18n, users.detail.deleting).to_string()
                                } else {
                                    t_string!(i18n, users.detail.confirmDelete).to_string()
                                }}
                            </Button>
                            <Button
                                on_click=move |_| set_show_delete_confirm.set(false)
                                class="flex-1 border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                disabled=Signal::derive(move || delete_form_state.get().is_submitting)
                            >
                                {move || t_string!(i18n, users.detail.cancel)}
                            </Button>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="rounded-2xl bg-card p-6 shadow border border-border">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, users.detail.section)}
                </h4>
                <Suspense
                    fallback=move || view! {
                        <p class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, users.detail.loading)}
                        </p>
                    }
                >
                    {move || match user_resource.get() {
                        None => view! {
                            <p class="text-sm text-muted-foreground">
                                {move || t_string!(i18n, users.detail.pending)}
                            </p>
                        }
                        .into_any(),
                        Some(Ok(response)) => {
                            if let Some(user) = response.user {
                                let email = user.email.clone();
                                let name_display = user.name.clone()
                                    .unwrap_or_else(|| t_string!(i18n, users.placeholderDash).to_string());
                                let role_display = user.role.clone();
                                let status_display = user.status.clone();
                                let tenant_display = user.tenant_name.clone()
                                    .unwrap_or_else(|| "—".to_string());
                                let created_at = user.created_at.clone();
                                let id = user.id.clone();

                                view! {
                                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {move || t_string!(i18n, users.detail.email)}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{email}</p>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {move || t_string!(i18n, users.detail.name)}
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
                                                {move || t_string!(i18n, users.detail.role)}
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
                                                {move || t_string!(i18n, users.detail.status)}
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
                                                {move || t_string!(i18n, users.detail.createdAt)}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{created_at}</p>
                                        </div>
                                        <div>
                                            <span class="text-xs text-muted-foreground">
                                                {move || t_string!(i18n, users.detail.id)}
                                            </span>
                                            <p class="mt-1 text-sm text-foreground">{id}</p>
                                        </div>
                                    </div>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                        {move || t_string!(i18n, users.detail.empty)}
                                    </div>
                                }
                                .into_any()
                            }
                        }
                        Some(Err(err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {err}
                            </div>
                        }
                        .into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
