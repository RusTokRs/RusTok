#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{GroupsAdminDirectory, GroupsAdminFilters, GroupsAdminListItem};

pub type GraphqlGroupsAdminError = String;

const DIRECTORY_QUERY: &str = "query GroupsAdminDirectory($page: Int, $perPage: Int, $search: String, $includeNonPublic: Boolean) { groups(page: $page, perPage: $perPage, search: $search, includeNonPublic: $includeNonPublic) { total page per_page: perPage items { id handle title visibility join_policy: joinPolicy status member_count: memberCount effective_locale: effectiveLocale } } }";

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    search: Option<String>,
    #[serde(rename = "includeNonPublic")]
    include_non_public: bool,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    groups: DirectoryWire,
}

#[derive(Debug, Deserialize)]
struct DirectoryWire {
    items: Vec<GroupWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct GroupWire {
    id: String,
    handle: String,
    title: String,
    visibility: String,
    join_policy: String,
    status: String,
    member_count: u64,
    effective_locale: String,
}

pub async fn load_directory(
    token: Option<String>,
    tenant_slug: Option<String>,
    filters: GroupsAdminFilters,
) -> Result<GroupsAdminDirectory, GraphqlGroupsAdminError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);
    let response: DirectoryResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            DIRECTORY_QUERY,
            Some(DirectoryVariables {
                page: page.min(i32::MAX as u64) as i32,
                per_page: per_page.min(i32::MAX as u64) as i32,
                search: filters.search,
                include_non_public: filters.include_non_public,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(GroupsAdminDirectory {
        items: response
            .groups
            .items
            .into_iter()
            .map(|group| GroupsAdminListItem {
                id: group.id,
                handle: group.handle,
                title: group.title,
                visibility: normalize_enum(group.visibility),
                join_policy: normalize_enum(group.join_policy),
                status: normalize_enum(group.status),
                member_count: group.member_count,
                effective_locale: group.effective_locale,
            })
            .collect(),
        total: response.groups.total,
        page: response.groups.page,
        per_page: response.groups.per_page,
    })
}

fn normalize_enum(value: String) -> String {
    value.to_ascii_lowercase()
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}
