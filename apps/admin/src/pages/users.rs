use base64::{engine::general_purpose::STANDARD, Engine};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_ui::{Badge, BadgeVariant};
use leptos_use::use_debounce_fn;
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::{CREATE_USER_MUTATION, USERS_QUERY, USERS_QUERY_HASH};
use crate::shared::api::{request, request_with_persisted, ApiError};
use crate::shared::ui::{Button, Input, LanguageToggle, PageHeader};
use crate::{t_string, use_i18n};

#[derive(Clone, Debug, Serialize)]
struct CreateUserVariables {
    input: CreateUserInput,
}

#[derive(Clone, Debug, Serialize)]
struct CreateUserInput {
    email: String,
    password: String,
    name: Option<String>,
    role: Option<String>,
    status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct CreateUserResponse {
    #[serde(rename = "createUser")]
    _create_user: Option<GraphqlUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlUsersResponse {
    users: GraphqlUsersConnection,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlUsersConnection {
    edges: Vec<GraphqlUserEdge>,
    #[serde(rename = "pageInfo")]
    page_info: GraphqlPageInfo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlUserEdge {
    node: GraphqlUser,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlPageInfo {
    #[serde(rename = "totalCount")]
    total_count: i64,
}

#[derive(Clone, Debug, Serialize)]
struct UsersVariables {
    pagination: PaginationInput,
    filter: Option<UsersFilterInput>,
    search: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct PaginationInput {
    first: i64,
    after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct UsersFilterInput {
    role: Option<String>,
    status: Option<String>,
}

fn cursor_for_page(page: i64, limit: i64) -> String {
    let index = ((page - 1) * limit).saturating_sub(1).max(0);
    STANDARD.encode(index.to_string())
}

fn users_table_skeleton() -> impl IntoView {
    view! {
        <div>
            <div class="mb-4 grid gap-3 md:grid-cols-3">
                {(0..3)
                    .map(|_| view! { <div class="h-12 animate-pulse rounded-xl bg-muted"></div> })
                    .collect_view()}
            </div>
            <div class="space-y-3">
                {(0..6)
                    .map(|_| view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> })
                    .collect_view()}
            </div>
            <div class="mt-4 flex items-center gap-3">
                <div class="h-9 w-24 animate-pulse rounded-lg bg-muted"></div>
                <div class="h-4 w-20 animate-pulse rounded bg-muted"></div>
                <div class="h-9 w-24 animate-pulse rounded-lg bg-muted"></div>
            </div>
        </div>
    }
}

#[component]
pub fn Users() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();
    let query = use_query_map();

    let initial_search = query.get_untracked().get("search").unwrap_or_default();
    let initial_role = query.get_untracked().get("role").unwrap_or_default();
    let initial_status = query.get_untracked().get("status").unwrap_or_default();
    let initial_page = query
        .get_untracked()
        .get("page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1);

    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (page, set_page) = signal(initial_page);
    let (limit, _set_limit) = signal(12i64);

    let (search_query, set_search_query) = signal(initial_search.clone());
    let (role_filter, set_role_filter) = signal(initial_role);
    let (status_filter, set_status_filter) = signal(initial_status);

    let (debounced_search, set_debounced_search) = signal(initial_search);
    let debounce_search = use_debounce_fn(
        move || set_debounced_search.set(search_query.get_untracked()),
        300.0,
    );
    Effect::new(move |_| {
        let _ = search_query.get();
        debounce_search();
    });

    Effect::new(move |_| {
        let _ = debounced_search.get();
        let _ = role_filter.get();
        let _ = status_filter.get();
        set_page.set(1);
    });

    Effect::new(move |_| {
        let s = debounced_search.get();
        let r = role_filter.get();
        let st = status_filter.get();
        let p = page.get();

        let mut params: Vec<(&str, String)> = Vec::new();
        if !s.is_empty() {
            params.push(("search", s));
        }
        if !r.is_empty() {
            params.push(("role", r));
        }
        if !st.is_empty() {
            params.push(("status", st));
        }
        if p > 1 {
            params.push(("page", p.to_string()));
        }

        let search_string = serde_urlencoded::to_string(params)
            .ok()
            .filter(|encoded| !encoded.is_empty())
            .map(|encoded| format!("?{}", encoded))
            .unwrap_or_default();

        navigate(&format!("/users{}", search_string), Default::default());
    });

    let graphql_resource = Resource::new(
        move || {
            (
                refresh_counter.get(),
                page.get(),
                limit.get(),
                debounced_search.get(),
                role_filter.get(),
                status_filter.get(),
            )
        },
        move |(_, page_val, limit_val, search_val, role_val, status_val)| {
            let token_value = token.get();
            let tenant_value = tenant.get();
            let after = if page_val > 1 {
                Some(cursor_for_page(page_val, limit_val))
            } else {
                None
            };
            async move {
                request_with_persisted::<UsersVariables, GraphqlUsersResponse>(
                    USERS_QUERY,
                    UsersVariables {
                        pagination: PaginationInput {
                            first: limit_val,
                            after,
                        },
                        filter: Some(UsersFilterInput {
                            role: if role_val.is_empty() {
                                None
                            } else {
                                Some(role_val.to_uppercase())
                            },
                            status: if status_val.is_empty() {
                                None
                            } else {
                                Some(status_val.to_uppercase())
                            },
                        }),
                        search: if search_val.is_empty() {
                            None
                        } else {
                            Some(search_val)
                        },
                    },
                    USERS_QUERY_HASH,
                    token_value,
                    tenant_value,
                )
                .await
            }
        },
    );

    let refresh = move |_| set_refresh_counter.update(|value| *value += 1);
    let next_page = move |_| set_page.update(|value| *value += 1);
    let previous_page = move |_| set_page.update(|value| *value = (*value - 1).max(1));

    let (show_create_modal, set_show_create_modal) = signal(false);
    let (new_email, set_new_email) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (new_name, set_new_name) = signal(String::new());
    let (new_role, set_new_role) = signal(String::new());
    let (new_status, set_new_status) = signal(String::new());
    let (create_error, set_create_error) = signal(Option::<String>::None);
    let (is_creating, set_is_creating) = signal(false);

    let open_create_modal = move |_| {
        set_new_email.set(String::new());
        set_new_password.set(String::new());
        set_new_name.set(String::new());
        set_new_role.set(String::new());
        set_new_status.set(String::new());
        set_create_error.set(None);
        set_show_create_modal.set(true);
    };

    let close_create_modal = move |_| {
        set_show_create_modal.set(false);
    };

    let create_user = {
        move |_| {
            let email_val = new_email.get();
            let password_val = new_password.get();
            let name_val = new_name.get();
            let role_val = new_role.get();
            let status_val = new_status.get();
            let token_val = token.get();
            let tenant_val = tenant.get();

            if email_val.is_empty() || password_val.is_empty() {
                set_create_error.set(Some(t_string!(i18n, users.create.errorRequired).to_string()));
                return;
            }

            set_is_creating.set(true);
            set_create_error.set(None);

            spawn_local(async move {
                let vars = CreateUserVariables {
                    input: CreateUserInput {
                        email: email_val,
                        password: password_val,
                        name: if name_val.is_empty() {
                            None
                        } else {
                            Some(name_val)
                        },
                        role: if role_val.is_empty() {
                            None
                        } else {
                            Some(role_val.to_uppercase())
                        },
                        status: if status_val.is_empty() {
                            None
                        } else {
                            Some(status_val.to_uppercase())
                        },
                    },
                };
                match request::<CreateUserVariables, CreateUserResponse>(
                    CREATE_USER_MUTATION,
                    vars,
                    token_val,
                    tenant_val,
                )
                .await
                {
                    Ok(_) => {
                        set_is_creating.set(false);
                        set_show_create_modal.set(false);
                        set_refresh_counter.update(|value| *value += 1);
                    }
                    Err(e) => {
                        set_is_creating.set(false);
                        set_create_error.set(Some(format!("{:?}", e)));
                    }
                }
            });
        }
    };

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title=t_string!(i18n, users.title)
                subtitle=t_string!(i18n, users.subtitle).to_string()
                eyebrow=t_string!(i18n, app.nav.users).to_string()
                actions=view! {
                    <LanguageToggle />
                    <Button
                        on_click=refresh
                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {move || t_string!(i18n, users.refresh)}
                    </Button>
                    <Button on_click=open_create_modal>
                        {move || t_string!(i18n, users.create.button)}
                    </Button>
                }
                .into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, users.graphql.title)}
                </h4>
                <Suspense
                    fallback=move || view! { <div>{users_table_skeleton()}</div> }
                >
                    {move || match graphql_resource.get() {
                        None => view! { <div>{users_table_skeleton()}</div> }.into_any(),
                        Some(Ok(response)) => {
                            let total_count = response.users.page_info.total_count;
                            let edges = response.users.edges;
                            view! {
                            <div>
                                <p class="text-xs text-muted-foreground mb-4">
                                    {move || t_string!(i18n, users.graphql.total)} " " {total_count}
                                </p>
                                <div class="mb-4 grid gap-3 md:grid-cols-3">
                                    <Input
                                        value=search_query
                                        set_value=set_search_query
                                        placeholder=move || t_string!(i18n, users.filters.searchPlaceholder)
                                        label=move || t_string!(i18n, users.filters.search)
                                    />
                                    <Input
                                        value=role_filter
                                        set_value=set_role_filter
                                        placeholder=move || t_string!(i18n, users.filters.rolePlaceholder)
                                        label=move || t_string!(i18n, users.filters.role)
                                    />
                                    <Input
                                        value=status_filter
                                        set_value=set_status_filter
                                        placeholder=move || t_string!(i18n, users.filters.statusPlaceholder)
                                        label=move || t_string!(i18n, users.filters.status)
                                    />
                                </div>
                                <div class="overflow-x-auto">
                                    <table class="w-full border-collapse text-sm">
                                        <thead>
                                            <tr>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {move || t_string!(i18n, users.graphql.email)}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {move || t_string!(i18n, users.graphql.name)}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {move || t_string!(i18n, users.graphql.role)}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {move || t_string!(i18n, users.graphql.status)}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {move || t_string!(i18n, users.graphql.createdAt)}
                                                </th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {{
                                                edges
                                                    .iter()
                                                    .map(|edge| {
                                                        let GraphqlUser {
                                                            id,
                                                            email,
                                                            name,
                                                            role,
                                                            status,
                                                            created_at,
                                                            ..
                                                        } = edge.node.clone();
                                                        view! {
                                                            <tr>
                                                                <td class="border-b border-border py-2">
                                                                    <A href=format!("/users/{}", id)>
                                                                        <span class="text-primary hover:underline">
                                                                            {email}
                                                                        </span>
                                                                    </A>
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">
                                                                    {name.unwrap_or_else(|| t_string!(i18n, users.placeholderDash).to_string())}
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">{role}</td>
                                                                <td class="border-b border-border py-2">
                                                                    <Badge variant=if status.eq_ignore_ascii_case("active") { BadgeVariant::Success } else { BadgeVariant::Default }>{status}</Badge>
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">{created_at}</td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                                <div class="mt-4 flex flex-wrap items-center gap-3">
                                    <Button
                                        on_click=previous_page
                                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                        disabled=Signal::derive(move || page.get() <= 1)
                                    >
                                        {move || t_string!(i18n, users.pagination.prev)}
                                    </Button>
                                    <span class="text-xs text-muted-foreground">
                                        {move || t_string!(i18n, users.pagination.page)} " " {page.get()}
                                    </span>
                                    <Button
                                        on_click=next_page
                                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                        disabled=Signal::derive(move || {
                                            let total = total_count;
                                            page.get() * limit.get() >= total
                                        })
                                    >
                                        {move || t_string!(i18n, users.pagination.next)}
                                    </Button>
                                </div>
                            </div>
                            }
                            .into_any()
                        }
                        Some(Err(err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {match err {
                                    ApiError::Unauthorized => t_string!(i18n, users.graphql.unauthorized).to_string(),
                                    ApiError::Http(code) => format!("{} {}", t_string!(i18n, users.graphql.error), code),
                                    ApiError::Network => t_string!(i18n, users.graphql.network).to_string(),
                                    ApiError::Graphql(message) => format!("{} {}", t_string!(i18n, users.graphql.error), message),
                                }}
                            </div>
                        }
                        .into_any(),
                    }}
                </Suspense>
            </div>

            <Show when=move || show_create_modal.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
                    <div class="w-full max-w-md rounded-2xl bg-card p-6 shadow-xl border border-border">
                        <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                            {move || t_string!(i18n, users.create.title)}
                        </h3>

                        <Show when=move || create_error.get().is_some()>
                            <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {move || create_error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        <div class="space-y-4">
                            <Input
                                value=new_email
                                set_value=set_new_email
                                placeholder="admin@rustok.io"
                                label=move || t_string!(i18n, users.create.emailLabel)
                            />
                            <Input
                                value=new_name
                                set_value=set_new_name
                                placeholder="John Doe"
                                label=move || t_string!(i18n, users.create.nameLabel)
                            />
                            <Input
                                value=new_password
                                set_value=set_new_password
                                placeholder="••••••••"
                                type_="password"
                                label=move || t_string!(i18n, users.create.passwordLabel)
                            />
                            <Input
                                value=new_role
                                set_value=set_new_role
                                placeholder="ADMIN, MANAGER, CUSTOMER"
                                label=move || t_string!(i18n, users.create.roleLabel)
                            />
                            <Input
                                value=new_status
                                set_value=set_new_status
                                placeholder="ACTIVE, INACTIVE"
                                label=move || t_string!(i18n, users.create.statusLabel)
                            />
                        </div>

                        <div class="mt-6 flex gap-3">
                            <Button
                                on_click=create_user
                                disabled=is_creating.into()
                                class="flex-1"
                            >
                                {move || if is_creating.get() {
                                    t_string!(i18n, users.create.creating).to_string()
                                } else {
                                    t_string!(i18n, users.create.submit).to_string()
                                }}
                            </Button>
                            <Button
                                on_click=close_create_modal
                                class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                            >
                                {move || t_string!(i18n, users.create.cancel)}
                            </Button>
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}
