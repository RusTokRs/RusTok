use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::ROLES_QUERY;
use crate::shared::api::{request, ApiError};
use crate::shared::ui::PageHeader;

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

#[component]
pub fn RolesPage() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();

    let roles_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            request::<EmptyVariables, GraphqlRolesResponse>(
                ROLES_QUERY,
                EmptyVariables {},
                token_value,
                tenant_value,
            )
            .await
        },
    );

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title="Roles & Permissions"
                subtitle="Platform roles and their permission sets".to_string()
                eyebrow="Access Control".to_string()
                actions=view! { <div /> }.into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">"Roles"</h4>
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
                                                        {permissions.len()} " permissions"
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
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {match err {
                                    ApiError::Unauthorized => "Unauthorized".to_string(),
                                    ApiError::Http(code) => format!("HTTP error {}", code),
                                    ApiError::Network => "Network error".to_string(),
                                    ApiError::Graphql(msg) => format!("GraphQL error: {}", msg),
                                }}
                            </div>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
