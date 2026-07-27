#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::core::NotificationStorefrontUnreadCount;

pub type GraphqlNotificationStorefrontError = String;

const UNREAD_COUNT_QUERY: &str = "query NotificationStorefrontNavigationUnreadCount { notificationInboxUnreadCount { unreadCount } }";

#[derive(Debug, Default, Serialize)]
struct EmptyVariables {}

#[derive(Debug, Deserialize)]
struct UnreadCountResponse {
    #[serde(rename = "notificationInboxUnreadCount")]
    unread_count: UnreadCountWire,
}

#[derive(Debug, Deserialize)]
struct UnreadCountWire {
    #[serde(rename = "unreadCount")]
    unread_count: u64,
}

pub async fn load_navigation_unread_count(
    access_token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<NotificationStorefrontUnreadCount, GraphqlNotificationStorefrontError> {
    let response: UnreadCountResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(UNREAD_COUNT_QUERY, Some(EmptyVariables::default())),
        access_token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(NotificationStorefrontUnreadCount {
        unread_count: response.unread_count.unread_count,
    })
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

#[cfg(test)]
mod tests {
    use super::UNREAD_COUNT_QUERY;

    #[test]
    fn unread_count_query_exposes_no_owner_identity_variables() {
        assert!(UNREAD_COUNT_QUERY.contains("notificationInboxUnreadCount"));
        assert!(UNREAD_COUNT_QUERY.contains("unreadCount"));
        assert!(!UNREAD_COUNT_QUERY.contains("tenantId"));
        assert!(!UNREAD_COUNT_QUERY.contains("recipientId"));
        assert!(!UNREAD_COUNT_QUERY.contains("userId"));
    }
}
