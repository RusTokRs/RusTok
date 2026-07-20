#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{MarketplaceSellerAdminEvent, MarketplaceSellerAdminEventHistory};

pub type GraphqlMarketplaceSellerEventHistoryError = String;

const EVENT_HISTORY_QUERY: &str = "query MarketplaceSellerEventHistory($sellerId: UUID!, $limit: Int!) { events: marketplaceSellerEvents(sellerId: $sellerId, limit: $limit) { id seller_id: sellerId actor_id: actorId event_kind: eventKind locale provenance note metadata created_at: createdAt } }";

#[derive(Debug, Serialize)]
struct EventHistoryVariables {
    #[serde(rename = "sellerId")]
    seller_id: String,
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct EventHistoryResponse {
    events: Vec<EventWire>,
}

#[derive(Debug, Deserialize)]
struct EventWire {
    id: String,
    seller_id: String,
    actor_id: Option<String>,
    event_kind: String,
    locale: Option<String>,
    provenance: String,
    note: Option<String>,
    metadata: serde_json::Value,
    created_at: String,
}

pub async fn load_event_history(
    token: Option<String>,
    tenant_slug: Option<String>,
    seller_id: String,
    limit: u64,
) -> Result<MarketplaceSellerAdminEventHistory, GraphqlMarketplaceSellerEventHistoryError> {
    let response: EventHistoryResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            EVENT_HISTORY_QUERY,
            Some(EventHistoryVariables {
                seller_id: seller_id.clone(),
                limit: limit.clamp(1, 200) as i32,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(MarketplaceSellerAdminEventHistory {
        seller_id,
        items: response
            .events
            .into_iter()
            .map(|event| MarketplaceSellerAdminEvent {
                id: event.id,
                seller_id: event.seller_id,
                actor_id: event.actor_id,
                event_kind: normalize_enum_output(event.event_kind),
                locale: event.locale,
                provenance: normalize_enum_output(event.provenance),
                note: event.note,
                metadata: event.metadata,
                created_at: event.created_at,
            })
            .collect(),
    })
}

fn normalize_enum_output(value: String) -> String {
    value.trim().to_ascii_lowercase()
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
