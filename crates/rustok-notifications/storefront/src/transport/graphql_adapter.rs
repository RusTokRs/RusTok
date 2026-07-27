#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::core::{
    NotificationStorefrontGroupItemsPage, NotificationStorefrontGroupItemsRequest,
    NotificationStorefrontGroupSummary, NotificationStorefrontGroupSummaryPage,
    NotificationStorefrontGroupSummaryRequest, NotificationStorefrontItem,
    NotificationStorefrontItemState, NotificationStorefrontPriority,
    NotificationStorefrontUnreadCount,
};

pub type GraphqlNotificationStorefrontError = String;

const UNREAD_COUNT_QUERY: &str = "query NotificationStorefrontNavigationUnreadCount { notificationInboxUnreadCount { unreadCount } }";
const GROUP_SUMMARIES_QUERY: &str = r#"
query NotificationStorefrontGroupSummaries($cursor: String, $limit: Int) {
  notificationInboxGroupSummaries(cursor: $cursor, limit: $limit) {
    groups {
      groupKey
      itemCount
      unreadCount
      latestItem {
        id
        source
        notificationType
        templateKey
        actorId
        priority
        state
        templateData { key value }
        seenAt
        readAt
        archivedAt
        createdAt
      }
    }
    nextCursor
    hasMore
  }
}
"#;
const GROUP_ITEMS_QUERY: &str = r#"
query NotificationStorefrontGroupItems(
  $groupKey: String!
  $state: NotificationInboxItemState
  $cursor: String
  $limit: Int
) {
  notificationInboxGroupItems(
    groupKey: $groupKey
    state: $state
    cursor: $cursor
    limit: $limit
  ) {
    items {
      id
      source
      notificationType
      templateKey
      actorId
      priority
      state
      templateData { key value }
      seenAt
      readAt
      archivedAt
      createdAt
    }
    nextCursor
    hasMore
  }
}
"#;

#[derive(Debug, Default, Serialize)]
struct EmptyVariables {}

#[derive(Debug, Serialize)]
struct GroupSummariesVariables {
    cursor: Option<String>,
    limit: i32,
}

#[derive(Debug, Serialize)]
struct GroupItemsVariables {
    #[serde(rename = "groupKey")]
    group_key: String,
    state: Option<GroupItemStateWire>,
    cursor: Option<String>,
    limit: i32,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GroupItemStateWire {
    Unread,
    Seen,
    Read,
    Archived,
}

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

#[derive(Debug, Deserialize)]
struct GroupSummariesResponse {
    #[serde(rename = "notificationInboxGroupSummaries")]
    page: GroupSummaryPageWire,
}

#[derive(Debug, Deserialize)]
struct GroupItemsResponse {
    #[serde(rename = "notificationInboxGroupItems")]
    page: GroupItemsPageWire,
}

#[derive(Debug, Deserialize)]
struct GroupSummaryPageWire {
    groups: Vec<GroupSummaryWire>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
    #[serde(rename = "hasMore")]
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct GroupItemsPageWire {
    items: Vec<ItemWire>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
    #[serde(rename = "hasMore")]
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct GroupSummaryWire {
    #[serde(rename = "groupKey")]
    group_key: String,
    #[serde(rename = "itemCount")]
    item_count: u64,
    #[serde(rename = "unreadCount")]
    unread_count: u64,
    #[serde(rename = "latestItem")]
    latest_item: ItemWire,
}

#[derive(Debug, Deserialize)]
struct ItemWire {
    id: String,
    source: String,
    #[serde(rename = "notificationType")]
    notification_type: String,
    #[serde(rename = "templateKey")]
    template_key: String,
    #[serde(rename = "actorId")]
    actor_id: Option<String>,
    priority: PriorityWire,
    state: ItemStateWire,
    #[serde(rename = "templateData")]
    template_data: Vec<TemplateFieldWire>,
    #[serde(rename = "seenAt")]
    seen_at: Option<String>,
    #[serde(rename = "readAt")]
    read_at: Option<String>,
    #[serde(rename = "archivedAt")]
    archived_at: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct TemplateFieldWire {
    key: String,
    value: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ItemStateWire {
    Unread,
    Seen,
    Read,
    Archived,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PriorityWire {
    Low,
    Normal,
    High,
    Urgent,
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

pub async fn load_group_summaries(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    request: NotificationStorefrontGroupSummaryRequest,
) -> Result<NotificationStorefrontGroupSummaryPage, GraphqlNotificationStorefrontError> {
    let response: GroupSummariesResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            GROUP_SUMMARIES_QUERY,
            Some(GroupSummariesVariables {
                cursor: request.cursor,
                limit: i32::from(request.limit),
            }),
        ),
        access_token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(NotificationStorefrontGroupSummaryPage {
        groups: response
            .page
            .groups
            .into_iter()
            .map(|group| NotificationStorefrontGroupSummary {
                group_key: group.group_key,
                item_count: group.item_count,
                unread_count: group.unread_count,
                latest_item: map_item(group.latest_item),
            })
            .collect(),
        next_cursor: response.page.next_cursor,
        has_more: response.page.has_more,
    })
}

pub async fn load_group_items(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    request: NotificationStorefrontGroupItemsRequest,
) -> Result<NotificationStorefrontGroupItemsPage, GraphqlNotificationStorefrontError> {
    let response: GroupItemsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            GROUP_ITEMS_QUERY,
            Some(GroupItemsVariables {
                group_key: request.group_key,
                state: request.state.map(map_state_to_wire),
                cursor: request.cursor,
                limit: i32::from(request.limit),
            }),
        ),
        access_token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(NotificationStorefrontGroupItemsPage {
        items: response.page.items.into_iter().map(map_item).collect(),
        next_cursor: response.page.next_cursor,
        has_more: response.page.has_more,
    })
}

fn map_item(item: ItemWire) -> NotificationStorefrontItem {
    NotificationStorefrontItem {
        id: item.id,
        source: item.source,
        notification_type: item.notification_type,
        template_key: item.template_key,
        actor_id: item.actor_id,
        priority: match item.priority {
            PriorityWire::Low => NotificationStorefrontPriority::Low,
            PriorityWire::Normal => NotificationStorefrontPriority::Normal,
            PriorityWire::High => NotificationStorefrontPriority::High,
            PriorityWire::Urgent => NotificationStorefrontPriority::Urgent,
        },
        state: match item.state {
            ItemStateWire::Unread => NotificationStorefrontItemState::Unread,
            ItemStateWire::Seen => NotificationStorefrontItemState::Seen,
            ItemStateWire::Read => NotificationStorefrontItemState::Read,
            ItemStateWire::Archived => NotificationStorefrontItemState::Archived,
        },
        template_data: item
            .template_data
            .into_iter()
            .map(|field| (field.key, field.value))
            .collect(),
        seen_at: item.seen_at,
        read_at: item.read_at,
        archived_at: item.archived_at,
        created_at: item.created_at,
    }
}

fn map_state_to_wire(state: NotificationStorefrontItemState) -> GroupItemStateWire {
    match state {
        NotificationStorefrontItemState::Unread => GroupItemStateWire::Unread,
        NotificationStorefrontItemState::Seen => GroupItemStateWire::Seen,
        NotificationStorefrontItemState::Read => GroupItemStateWire::Read,
        NotificationStorefrontItemState::Archived => GroupItemStateWire::Archived,
    }
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
    use super::{GROUP_ITEMS_QUERY, GROUP_SUMMARIES_QUERY, UNREAD_COUNT_QUERY};

    #[test]
    fn inbox_queries_expose_no_owner_identity_variables() {
        for query in [UNREAD_COUNT_QUERY, GROUP_SUMMARIES_QUERY, GROUP_ITEMS_QUERY] {
            for forbidden in [
                ["tenant", "Id"].concat(),
                ["recipient", "Id"].concat(),
                ["user", "Id"].concat(),
            ] {
                assert!(!query.contains(forbidden.as_str()));
            }
        }
    }

    #[test]
    fn grouped_queries_keep_bounded_paging_and_typed_state_contracts() {
        assert!(GROUP_SUMMARIES_QUERY.contains("$cursor: String"));
        assert!(GROUP_SUMMARIES_QUERY.contains("$limit: Int"));
        assert!(GROUP_ITEMS_QUERY.contains("$groupKey: String!"));
        assert!(GROUP_ITEMS_QUERY.contains("$state: NotificationInboxItemState"));
        assert!(GROUP_ITEMS_QUERY.contains("templateData { key value }"));
    }
}
