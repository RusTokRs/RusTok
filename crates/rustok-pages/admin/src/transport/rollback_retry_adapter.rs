use fly::ProjectHash;
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{PageDetail, RollbackPageReceipt};

use super::graphql_adapter;

const ROLLBACK_PAGE_MUTATION: &str = "mutation RollbackPage($id: UUID!, $input: RollbackGqlPageInput!) { rollbackPage(id: $id, input: $input) { operationId pageId version idempotencyKey targetPublishOperationId sourceArtifactSetHash targetArtifactSetHash replayed rolledBackAt } }";
const ROLLBACK_IDEMPOTENCY_FORMAT: &str = "pages_admin_rollback_v1";
const ROLLBACK_RETRY_STORAGE_PREFIX: &str = "rustok.pages.rollback.pending.v1";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PendingRollbackAttempt {
    expected_version: i32,
    idempotency_key: String,
}

#[derive(Debug, Serialize)]
struct RollbackVariables {
    id: String,
    input: RollbackInput,
}

#[derive(Debug, Serialize)]
struct RollbackInput {
    #[serde(rename = "expectedVersion")]
    expected_version: i32,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct RollbackResponse {
    #[serde(rename = "rollbackPage")]
    rollback_page: RollbackPageReceipt,
}

pub(super) async fn rollback_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
) -> Result<RollbackPageReceipt, GraphqlHttpError> {
    let attempt = match load_pending_attempt(&page_id)? {
        Some(attempt) => attempt,
        None => {
            let page = graphql_adapter::fetch_page(
                token.clone(),
                tenant_slug.clone(),
                page_id.clone(),
            )
            .await?
            .ok_or_else(|| GraphqlHttpError::Graphql("Page was not found".to_string()))?;
            if page.status != "published" {
                return Err(GraphqlHttpError::Graphql(
                    "Only a currently published page can be rolled back".to_string(),
                ));
            }
            let attempt = PendingRollbackAttempt {
                expected_version: page.version,
                idempotency_key: rollback_idempotency_key(&page)?,
            };
            store_pending_attempt(&page_id, &attempt)?;
            attempt
        }
    };

    let result: Result<RollbackResponse, GraphqlHttpError> = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            ROLLBACK_PAGE_MUTATION,
            Some(RollbackVariables {
                id: page_id.clone(),
                input: RollbackInput {
                    expected_version: attempt.expected_version,
                    idempotency_key: attempt.idempotency_key.clone(),
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await;

    match result {
        Ok(response) => {
            let _ = clear_pending_attempt(&page_id);
            Ok(response.rollback_page)
        }
        Err(error) => {
            if is_definitive_rejection(&error) {
                let _ = clear_pending_attempt(&page_id);
            }
            Err(error)
        }
    }
}

fn rollback_idempotency_key(page: &PageDetail) -> Result<String, GraphqlHttpError> {
    let bytes = serde_json::to_vec(&(
        ROLLBACK_IDEMPOTENCY_FORMAT,
        page.id.as_str(),
        page.version,
    ))
    .map_err(|error| {
        GraphqlHttpError::Graphql(format!("Unable to encode page rollback identity: {error}"))
    })?;
    Ok(format!(
        "pages-rollback-v1:{}:{}:{}",
        page.id,
        page.version,
        ProjectHash::from_bytes(&bytes).hex()
    ))
}

fn is_definitive_rejection(error: &GraphqlHttpError) -> bool {
    match error {
        GraphqlHttpError::Graphql(message) => {
            let message = message.to_ascii_lowercase();
            [
                "idempotency conflict",
                "target unavailable",
                "requires a published page",
                "only a currently published page",
                "version conflict",
                "validation error",
                "permission denied",
                "forbidden",
                "not found",
            ]
            .iter()
            .any(|marker| message.contains(marker))
        }
        GraphqlHttpError::Unauthorized => true,
        GraphqlHttpError::Http(status) => status.trim_start().starts_with('4'),
        GraphqlHttpError::Network => false,
    }
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_else(|| "http://localhost:5150".to_string());
    format!("{origin}/api/graphql")
}

fn retry_storage() -> Result<web_sys::Storage, GraphqlHttpError> {
    web_sys::window()
        .ok_or_else(|| {
            GraphqlHttpError::Graphql(
                "Rollback requires a browser window for durable retry identity".to_string(),
            )
        })?
        .session_storage()
        .map_err(|_| {
            GraphqlHttpError::Graphql(
                "Unable to access browser session storage for rollback retry identity".to_string(),
            )
        })?
        .ok_or_else(|| {
            GraphqlHttpError::Graphql(
                "Browser session storage is unavailable for rollback retry identity".to_string(),
            )
        })
}

fn retry_storage_key(page_id: &str) -> String {
    format!("{ROLLBACK_RETRY_STORAGE_PREFIX}:{page_id}")
}

fn load_pending_attempt(
    page_id: &str,
) -> Result<Option<PendingRollbackAttempt>, GraphqlHttpError> {
    let storage = retry_storage()?;
    let key = retry_storage_key(page_id);
    let Some(raw) = storage.get_item(&key).map_err(|_| {
        GraphqlHttpError::Graphql(
            "Unable to read rollback retry identity from session storage".to_string(),
        )
    })?
    else {
        return Ok(None);
    };
    let attempt = match serde_json::from_str::<PendingRollbackAttempt>(&raw) {
        Ok(attempt)
            if attempt.expected_version > 0 && !attempt.idempotency_key.trim().is_empty() =>
        {
            attempt
        }
        _ => {
            storage.remove_item(&key).map_err(|_| {
                GraphqlHttpError::Graphql(
                    "Unable to clear invalid rollback retry identity".to_string(),
                )
            })?;
            return Ok(None);
        }
    };
    Ok(Some(attempt))
}

fn store_pending_attempt(
    page_id: &str,
    attempt: &PendingRollbackAttempt,
) -> Result<(), GraphqlHttpError> {
    let raw = serde_json::to_string(attempt).map_err(|error| {
        GraphqlHttpError::Graphql(format!(
            "Unable to encode rollback retry identity: {error}"
        ))
    })?;
    retry_storage()?
        .set_item(&retry_storage_key(page_id), &raw)
        .map_err(|_| {
            GraphqlHttpError::Graphql(
                "Unable to persist rollback retry identity before sending the request".to_string(),
            )
        })
}

fn clear_pending_attempt(page_id: &str) -> Result<(), GraphqlHttpError> {
    retry_storage()?
        .remove_item(&retry_storage_key(page_id))
        .map_err(|_| {
            GraphqlHttpError::Graphql(
                "Unable to clear rollback retry identity from session storage".to_string(),
            )
        })
}
