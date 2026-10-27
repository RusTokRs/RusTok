//! Canonical semantic identity for durable idempotent commands.
//!
//! Durable replay is keyed by an explicit scope and idempotency key. The
//! digest therefore covers only the semantic command object. Top-level
//! trace, correlation, and free-text reason fields are audit/observability
//! evidence from one attempt; they are deliberately not replay identity. A
//! top-level command context is likewise removed, except that its `actor_id`
//! is retained as an explicit replay-principal boundary for the durable
//! management/data commands that use this helper. Nested payload fields are
//! untouched because a nested `reason` or `context` may be part of the
//! caller's actual data.

use serde::Serialize;
use sha2::{Digest, Sha256};

const NON_SEMANTIC_TOP_LEVEL_FIELDS: [&str; 4] =
    ["context", "trace_id", "correlation_id", "reason"];

/// Computes the canonical SHA-256 digest of a command's semantic fields.
pub(crate) fn semantic_request_digest<T: Serialize>(
    request: &T,
) -> Result<String, serde_json::Error> {
    let mut identity = serde_json::to_value(request)?;
    if let Some(object) = identity.as_object_mut() {
        let context_actor_id = object
            .get("context")
            .and_then(|context| context.get("actor_id"))
            .cloned();
        for field in NON_SEMANTIC_TOP_LEVEL_FIELDS {
            object.remove(field);
        }
        if let Some(actor_id) = context_actor_id {
            object.entry("actor_id".to_string()).or_insert(actor_id);
        }
    }
    let bytes = rustok_api::manifest_hash::canonical_json_bytes(&identity)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::semantic_request_digest;

    #[derive(Serialize)]
    struct Context<'a> {
        actor_id: &'a str,
        trace_id: &'a str,
        correlation_id: &'a str,
    }

    #[derive(Serialize)]
    struct Command<'a> {
        target: &'a str,
        expected_revision: u64,
        context: Context<'a>,
        reason: &'a str,
        payload: serde_json::Value,
    }

    #[test]
    fn observability_fields_do_not_change_semantic_identity() {
        let first = Command {
            target: "module-a",
            expected_revision: 3,
            context: Context {
                actor_id: "actor-1",
                trace_id: "trace-1",
                correlation_id: "correlation-1",
            },
            reason: "first attempt",
            payload: serde_json::json!({ "reason": "payload-owned value" }),
        };
        let retry = Command {
            target: "module-a",
            expected_revision: 3,
            context: Context {
                actor_id: "actor-1",
                trace_id: "trace-2",
                correlation_id: "correlation-2",
            },
            reason: "retry attempt",
            payload: serde_json::json!({ "reason": "payload-owned value" }),
        };

        assert_eq!(
            semantic_request_digest(&first).expect("first digest"),
            semantic_request_digest(&retry).expect("retry digest")
        );
    }

    #[test]
    fn semantic_and_nested_payload_changes_remain_distinct() {
        let first = Command {
            target: "module-a",
            expected_revision: 3,
            context: Context {
                actor_id: "actor-1",
                trace_id: "trace-1",
                correlation_id: "correlation-1",
            },
            reason: "first attempt",
            payload: serde_json::json!({ "reason": "payload-owned value" }),
        };
        let changed_revision = Command {
            target: "module-a",
            expected_revision: 4,
            context: Context {
                actor_id: "actor-1",
                trace_id: "trace-1",
                correlation_id: "correlation-1",
            },
            reason: "first attempt",
            payload: serde_json::json!({ "reason": "payload-owned value" }),
        };
        let changed_payload = Command {
            target: "module-a",
            expected_revision: 3,
            context: Context {
                actor_id: "actor-1",
                trace_id: "trace-1",
                correlation_id: "correlation-1",
            },
            reason: "first attempt",
            payload: serde_json::json!({ "reason": "different payload value" }),
        };

        assert_ne!(
            semantic_request_digest(&first).expect("first digest"),
            semantic_request_digest(&changed_revision).expect("changed revision digest")
        );
        assert_ne!(
            semantic_request_digest(&first).expect("first digest"),
            semantic_request_digest(&changed_payload).expect("changed payload digest")
        );

        let changed_actor = Command {
            target: "module-a",
            expected_revision: 3,
            context: Context {
                actor_id: "actor-2",
                trace_id: "trace-1",
                correlation_id: "correlation-1",
            },
            reason: "first attempt",
            payload: serde_json::json!({ "reason": "payload-owned value" }),
        };
        assert_ne!(
            semantic_request_digest(&first).expect("first digest"),
            semantic_request_digest(&changed_actor).expect("changed actor digest")
        );
    }
}
