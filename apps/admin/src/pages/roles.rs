use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::ROLES_QUERY;
use crate::shared::api::{request, ApiError};
use crate::shared::ui::PageHeader;
use crate::{t_string, use_i18n};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

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
    match selected_transport_path() {
        UiTransportPath::Graphql => match fetch_roles_graphql(token, tenant_slug).await {
            Ok(response) => Ok(response),
            Err(graphql_err) => Ok(built_in_roles_response(Some(graphql_err.to_string()))),
        },
        UiTransportPath::NativeServer => fetch_roles_server()
            .await
            .map_err(|error| error.to_string()),
    }
}

fn built_in_roles_response(_source_error: Option<String>) -> GraphqlRolesResponse {
    let mut roles = vec![
        RoleInfo {
            slug: "super_admin".to_string(),
            display_name: "Super Admin".to_string(),
            permissions: manage_permissions(&[
                "users",
                "tenants",
                "modules",
                "settings",
                "flex_schemas",
                "flex_entries",
                "products",
                "categories",
                "orders",
                "customers",
                "inventory",
                "discounts",
                "posts",
                "pages",
                "nodes",
                "media",
                "seo",
                "comments",
                "taxonomy",
                "analytics",
                "logs",
                "webhooks",
                "scripts",
                "mcp",
                "ai:providers",
                "ai:task_profiles",
                "ai:sessions",
                "ai:runs",
                "ai:approvals",
                "ai:router",
                "ai:tasks:text",
                "ai:tasks:image",
                "ai:tasks:code",
                "ai:tasks:alloy",
                "ai:tasks:multimodal",
                "blog_posts",
                "tags",
                "forum_categories",
                "forum_topics",
                "forum_replies",
                "workflows",
                "workflow_executions",
            ]),
        },
        RoleInfo {
            slug: "admin".to_string(),
            display_name: "Admin".to_string(),
            permissions: sorted_permissions(vec![
                manage_permissions(&[
                    "users",
                    "settings",
                    "products",
                    "categories",
                    "orders",
                    "customers",
                    "inventory",
                    "discounts",
                    "posts",
                    "pages",
                    "nodes",
                    "media",
                    "seo",
                    "comments",
                    "taxonomy",
                    "analytics",
                    "webhooks",
                    "scripts",
                    "mcp",
                    "ai:providers",
                    "ai:task_profiles",
                    "ai:sessions",
                    "ai:runs",
                    "ai:approvals",
                    "ai:router",
                    "ai:tasks:text",
                    "ai:tasks:image",
                    "ai:tasks:code",
                    "ai:tasks:alloy",
                    "ai:tasks:multimodal",
                    "blog_posts",
                    "tags",
                    "forum_categories",
                    "forum_topics",
                    "forum_replies",
                    "workflows",
                    "workflow_executions",
                ]),
                expand_permissions(&["modules", "logs"], &["read", "list"]),
                expand_permissions(&["flex_schemas"], &["create", "read", "update", "list"]),
            ]),
        },
        RoleInfo {
            slug: "manager".to_string(),
            display_name: "Manager".to_string(),
            permissions: sorted_permissions(vec![
                expand_permissions(
                    &[
                        "products",
                        "categories",
                        "posts",
                        "nodes",
                        "media",
                        "taxonomy",
                        "pages",
                    ],
                    &["create", "read", "update", "delete", "list"],
                ),
                expand_permissions(&["orders"], &["read", "update", "list"]),
                expand_permissions(&["customers"], &["read", "list"]),
                expand_permissions(&["inventory"], &["create", "read", "update", "list"]),
                expand_permissions(
                    &["blog_posts"],
                    &["create", "read", "update", "delete", "list", "publish"],
                ),
                expand_permissions(&["seo"], &["read", "update", "publish", "execute"]),
                expand_permissions(&["ai:providers", "ai:task_profiles"], &["read"]),
                expand_permissions(&["ai:sessions"], &["read", "run"]),
                expand_permissions(&["ai:runs"], &["cancel"]),
                expand_permissions(&["ai:approvals"], &["resolve"]),
                expand_permissions(
                    &[
                        "ai:tasks:text",
                        "ai:tasks:image",
                        "ai:tasks:code",
                        "ai:tasks:alloy",
                        "ai:tasks:multimodal",
                    ],
                    &["run"],
                ),
                expand_permissions(&["forum_categories"], &["create", "read", "update", "list"]),
                expand_permissions(
                    &["forum_topics", "forum_replies"],
                    &["create", "read", "update", "delete", "list", "moderate"],
                ),
                expand_permissions(&["analytics"], &["read"]),
            ]),
        },
        RoleInfo {
            slug: "customer".to_string(),
            display_name: "Customer".to_string(),
            permissions: sorted_permissions(vec![
                expand_permissions(
                    &[
                        "products",
                        "categories",
                        "posts",
                        "nodes",
                        "pages",
                        "taxonomy",
                    ],
                    &["read", "list"],
                ),
                expand_permissions(&["orders"], &["create", "read", "list"]),
                expand_permissions(&["comments"], &["create", "read", "list"]),
                expand_permissions(&["blog_posts", "forum_categories"], &["read", "list"]),
                expand_permissions(
                    &["forum_topics", "forum_replies"],
                    &["create", "read", "list"],
                ),
                expand_permissions(&["inventory"], &["read", "list"]),
            ]),
        },
    ];

    roles.sort_by_key(|role| role_sort_key(&role.slug));
    GraphqlRolesResponse { roles }
}

fn manage_permissions(resources: &[&str]) -> Vec<String> {
    expand_permissions(resources, &["manage"])
}

fn expand_permissions(resources: &[&str], actions: &[&str]) -> Vec<String> {
    resources
        .iter()
        .flat_map(|resource| {
            actions
                .iter()
                .map(move |action| format!("{}:{}", resource, action))
        })
        .collect()
}

fn sorted_permissions(groups: Vec<Vec<String>>) -> Vec<String> {
    groups
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Debug)]
struct PermissionGroup {
    name: String,
    permissions: Vec<String>,
}

fn role_sort_key(slug: &str) -> usize {
    match slug {
        "super_admin" => 0,
        "admin" => 1,
        "manager" => 2,
        "customer" => 3,
        _ => 99,
    }
}

fn permission_family(permission: &str) -> &'static str {
    let resource = permission
        .rsplit_once(':')
        .map(|(resource, _)| resource)
        .unwrap_or(permission);

    match resource {
        "users" | "tenants" | "settings" | "profiles" => "Access",
        "modules" | "logs" | "webhooks" | "scripts" | "mcp" => "Platform",
        "products" | "categories" | "orders" | "customers" | "inventory" | "discounts"
        | "payments" | "fulfillments" | "regions" => "Commerce",
        "posts" | "pages" | "nodes" | "media" | "seo" | "comments" | "tags" | "taxonomy"
        | "blog_posts" | "forum_categories" | "forum_topics" | "forum_replies" => "Content",
        "analytics" | "flex_schemas" | "flex_entries" => "Runtime",
        "workflows" | "workflow_executions" => "Automation",
        value if value.starts_with("ai:") => "AI",
        _ => "Other",
    }
}

fn group_permissions(permissions: &[String]) -> Vec<PermissionGroup> {
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for permission in permissions {
        groups
            .entry(permission_family(permission).to_string())
            .or_default()
            .push(permission.clone());
    }

    groups
        .into_iter()
        .map(|(name, mut permissions)| {
            permissions.sort();
            PermissionGroup { name, permissions }
        })
        .collect()
}

#[server(prefix = "/api/fn", endpoint = "admin/list-roles")]
async fn list_roles_native() -> Result<GraphqlRolesResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::extract;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext};
        use rustok_core::{Rbac, UserRole};

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
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_string!(i18n, roles.title)
                subtitle=t_string!(i18n, roles.subtitle).to_string()
                eyebrow=t_string!(i18n, roles.eyebrow).to_string()
            />

            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
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
                            let mut roles = response.roles;
                            roles.sort_by_key(|role| role_sort_key(&role.slug));
                            let roles_count = roles.len();
                            let unique_permissions = roles
                                .iter()
                                .flat_map(|role| role.permissions.iter().cloned())
                                .collect::<BTreeSet<_>>()
                                .len();
                            view! {
                                <div class="space-y-5">
                                    <div class="grid gap-3 md:grid-cols-3">
                                        <div class="rounded-lg border border-border bg-background p-4">
                                            <p class="text-xs font-medium text-muted-foreground">
                                                {move || t_string!(i18n, roles.summary.roles)}
                                            </p>
                                            <p class="mt-2 text-2xl font-semibold text-foreground">{roles_count}</p>
                                        </div>
                                        <div class="rounded-lg border border-border bg-background p-4">
                                            <p class="text-xs font-medium text-muted-foreground">
                                                {move || t_string!(i18n, roles.summary.permissions)}
                                            </p>
                                            <p class="mt-2 text-2xl font-semibold text-foreground">{unique_permissions}</p>
                                        </div>
                                        <div class="rounded-lg border border-border bg-background p-4">
                                            <p class="text-xs font-medium text-muted-foreground">
                                                {move || t_string!(i18n, roles.summary.source)}
                                            </p>
                                            <p class="mt-2 text-sm font-medium text-foreground">
                                                {move || t_string!(i18n, roles.summary.system)}
                                            </p>
                                        </div>
                                    </div>
                                    {roles.into_iter().map(|role| {
                                        let slug = role.slug.clone();
                                        let display_name = role.display_name.clone();
                                        let permissions = role.permissions.clone();
                                        let perm_count = permissions.len();
                                        let permission_groups = group_permissions(&permissions);
                                        let description = match slug.as_str() {
                                            "super_admin" => t_string!(i18n, roles.description.superAdmin).to_string(),
                                            "admin" => t_string!(i18n, roles.description.admin).to_string(),
                                            "manager" => t_string!(i18n, roles.description.manager).to_string(),
                                            "customer" => t_string!(i18n, roles.description.customer).to_string(),
                                            _ => t_string!(i18n, roles.description.custom).to_string(),
                                        };
                                        view! {
                                            <div class="rounded-xl border border-border bg-background p-4 shadow-sm">
                                                <div class="flex flex-wrap items-start gap-3">
                                                    <div class="min-w-0 flex-1">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <span class="text-sm font-semibold text-foreground">
                                                                {display_name}
                                                            </span>
                                                            <span class="inline-flex items-center rounded-md bg-primary/10 px-2.5 py-0.5 font-mono text-[11px] font-semibold text-primary">
                                                                {slug}
                                                            </span>
                                                        </div>
                                                        <p class="mt-1 text-sm text-muted-foreground">
                                                            {description}
                                                        </p>
                                                    </div>
                                                    <div class="rounded-lg border border-border bg-card px-3 py-2 text-right">
                                                        <p class="text-lg font-semibold leading-none text-card-foreground">
                                                            {perm_count}
                                                        </p>
                                                        <p class="mt-1 text-[11px] uppercase tracking-wide text-muted-foreground">
                                                            {move || t_string!(i18n, roles.permissions)}
                                                        </p>
                                                    </div>
                                                </div>

                                                <div class="mt-4 grid gap-3 lg:grid-cols-2">
                                                    {permission_groups.into_iter().map(|group| {
                                                        let group_name = group.name;
                                                        let group_count = group.permissions.len();
                                                        let group_permissions = group.permissions;
                                                        view! {
                                                            <div class="rounded-lg border border-border/80 bg-card p-3">
                                                                <div class="mb-2 flex items-center justify-between gap-2">
                                                                    <span class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                                        {group_name}
                                                                    </span>
                                                                    <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                                        {group_count}
                                                                    </span>
                                                                </div>
                                                                <div class="flex flex-wrap gap-1.5">
                                                                    {group_permissions.into_iter().map(|perm| view! {
                                                                        <span class="inline-flex max-w-full items-center rounded border border-border bg-muted/70 px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                                                                            {perm}
                                                                        </span>
                                                                    }).collect_view()}
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                        Some(Err(_err)) => view! {
                            <div class="rounded-xl border border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {move || t_string!(i18n, roles.error)}
                            </div>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
