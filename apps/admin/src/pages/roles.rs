use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::ROLES_QUERY;
use crate::shared::api::{request, ApiError};
use crate::shared::ui::{Alert, AlertVariant, PageHeader};
use crate::{t_string, use_i18n};

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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlRolesResponse {
    roles: Vec<RoleInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RoleInfo {
    slug: String,
    #[serde(rename = "displayName")]
    display_name: String,
    permissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

async fn fetch_roles_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlRolesResponse, ApiError> {
    request::<EmptyVariables, GraphqlRolesResponse>(
        ROLES_QUERY,
        EmptyVariables {},
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_roles_server() -> Result<GraphqlRolesResponse, ServerFnError> {
    list_roles_native().await
}

async fn fetch_roles(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlRolesResponse, String> {
    match fetch_roles_server().await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_roles_graphql(token, tenant_slug)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-roles")]
async fn list_roles_native() -> Result<GraphqlRolesResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::extract;
        use rustok_api::{has_effective_permission, AuthContext};
        use rustok_core::{Permission, Rbac, UserRole};

        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required to list roles"));
        }

        let roles = [
            UserRole::SuperAdmin,
            UserRole::Admin,
            UserRole::Manager,
            UserRole::Customer,
        ]
        .into_iter()
        .map(|role| {
            let mut permissions = Rbac::permissions_for_role(&role)
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>();
            permissions.sort();

            let display_name = match role {
                UserRole::SuperAdmin => "Super Admin",
                UserRole::Admin => "Admin",
                UserRole::Manager => "Manager",
                UserRole::Customer => "Customer",
            };

            RoleInfo {
                slug: role.to_string(),
                display_name: display_name.to_string(),
                permissions,
            }
        })
        .collect();

        Ok(GraphqlRolesResponse { roles })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-roles requires the `ssr` feature",
        ))
    }
}

#[component]
pub fn RolesPage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let roles_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move { fetch_roles(token_value, tenant_value).await },
    );

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title=t_string!(i18n, roles.title)
                subtitle=t_string!(i18n, roles.subtitle).to_string()
                eyebrow=t_string!(i18n, roles.eyebrow).to_string()
                actions=view! { <div /> }.into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, roles.list.title)}
                </h4>
                <Suspense fallback=move || view! {
                    <div class="space-y-3">
                        {(0..4).map(|_| view! {
                            <div class="h-12 animate-pulse rounded-lg bg-muted" />
                        }).collect_view()}
                    </div>
                }>
                    {move || match roles_resource.get() {
                        None => view! {
                            <div class="space-y-3">
                                {(0..4).map(|_| view! {
                                    <div class="h-12 animate-pulse rounded-lg bg-muted" />
                                }).collect_view()}
                            </div>
                        }.into_any(),
                        Some(Ok(response)) => {
                            let roles = response.roles;
                            view! {
                                <div class="space-y-4">
                                    {roles.into_iter().map(|role| {
                                        let slug = role.slug.clone();
                                        let display_name = role.display_name.clone();
                                        let permissions = role.permissions.clone();
                                        let perm_count = permissions.len();
                                        view! {
                                            <div class="rounded-xl border border-border bg-background p-4">
                                                <div class="flex items-center gap-3 mb-2">
                                                    <span class="inline-flex items-center rounded-md px-2.5 py-0.5 text-xs font-semibold bg-primary/10 text-primary">
                                                        {slug}
                                                    </span>
                                                    <span class="text-sm font-medium text-foreground">
                                                        {display_name}
                                                    </span>
                                                    <span class="ml-auto text-xs text-muted-foreground">
                                                        {perm_count}
                                                        " "
                                                        {t_string!(i18n, roles.permissions)}
                                                    </span>
                                                </div>
                                                <div class="flex flex-wrap gap-1.5">
                                                    {permissions.into_iter().map(|perm| view! {
                                                        <span class="inline-flex items-center rounded px-2 py-0.5 text-xs bg-muted text-muted-foreground font-mono">
                                                            {perm}
                                                        </span>
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <Alert variant=AlertVariant::Destructive>
                                {err.to_string()}
                            </Alert>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
