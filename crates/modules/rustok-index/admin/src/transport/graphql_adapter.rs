use rustok_graphql::{
    GraphqlHttpError, GraphqlOperationError, GraphqlRequest, execute_with_metadata, graphql_url,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::model::{
    CancelActionResult, CancelActionStatus, CancelJobInput, ReplayActionResult, RetryActionResult,
    RetryActionStatus, RetryJobInput, RetryJobKind, TriggerReplayInput,
};

const RUN: &str = include_str!("graphql/run.graphql");
const CANCEL: &str = include_str!("graphql/cancel.graphql");
const REQUEUE_REPLAY: &str = include_str!("graphql/requeue_replay.graphql");
const REQUEUE_RECONCILIATION: &str = include_str!("graphql/requeue_reconciliation.graphql");
const UNKNOWN_RESULT: &str = "Index command result unavailable; refresh status before retrying";

#[derive(Deserialize)]
struct Response<T> {
    result: T,
}

async fn command<T: DeserializeOwned>(
    document: &'static str,
    input: Value,
    token: Option<String>,
    tenant: Option<String>,
) -> Result<T, &'static str> {
    let Response { result } = execute_with_metadata::<_, Response<T>>(
        &graphql_url(),
        GraphqlRequest::new(document, Some(json!({ "input": input }))),
        token,
        tenant,
        None,
    )
    .await
    .map_err(command_error)?;
    Ok(result)
}

fn command_error(error: GraphqlOperationError) -> &'static str {
    if error.error == GraphqlHttpError::Unauthorized {
        return "Index authentication required";
    }
    match error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.code.as_deref())
    {
        Some("UNAUTHENTICATED" | "UNAUTHORIZED") => "Index authentication required",
        Some("FORBIDDEN" | "PERMISSION_DENIED") => "modules:manage required for Index jobs",
        Some("BAD_USER_INPUT") => "Invalid Index command input",
        Some("INDEX_REPLAY_DEAD_LETTERED") => {
            "Replay is dead-lettered; resolve the failure and requeue the original job"
        }
        _ => UNKNOWN_RESULT,
    }
}

pub(super) async fn trigger_replay(
    token: Option<String>,
    tenant: Option<String>,
    input: TriggerReplayInput,
) -> Result<ReplayActionResult, &'static str> {
    command(
        RUN,
        json!({
            "moduleName": input.schema_module.trim(),
            "entityName": input.schema_entity.trim(),
            "schemaVersion": input.schema_version.to_string(),
            "locale": input.locale.as_deref().map(str::trim),
        }),
        token,
        tenant,
    )
    .await
}

fn job_id(value: &str) -> Result<Uuid, &'static str> {
    Uuid::parse_str(value.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or("Job ID must be a non-nil UUID")
}

pub(super) async fn cancel_job(
    token: Option<String>,
    tenant: Option<String>,
    input: CancelJobInput,
) -> Result<CancelActionResult, &'static str> {
    #[derive(Deserialize)]
    struct Payload {
        status: CancelActionStatus,
    }
    let id = job_id(&input.job_id)?;
    let result: Payload = command(CANCEL, json!({ "jobId": id }), token, tenant).await?;
    Ok(CancelActionResult {
        job_id: id.to_string(),
        outcome: result.status,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequeuePayload {
    status: RetryActionStatus,
    retry_epoch: Option<u32>,
}

impl RequeuePayload {
    fn into_result(self, id: Uuid) -> Result<RetryActionResult, &'static str> {
        match (self.status, self.retry_epoch) {
            (RetryActionStatus::Requeued, Some(epoch)) if epoch > 0 => {}
            (
                RetryActionStatus::NotFound
                | RetryActionStatus::NotFailed
                | RetryActionStatus::ScopeUnavailable,
                None,
            ) => {}
            _ => return Err(UNKNOWN_RESULT),
        }
        Ok(RetryActionResult {
            job_id: id.to_string(),
            outcome: self.status,
            retry_epoch: self.retry_epoch,
        })
    }
}

const fn recovery_document(kind: RetryJobKind) -> &'static str {
    match kind {
        RetryJobKind::Replay => REQUEUE_REPLAY,
        RetryJobKind::Reconciliation => REQUEUE_RECONCILIATION,
    }
}

pub(super) async fn retry_job(
    token: Option<String>,
    tenant: Option<String>,
    input: RetryJobInput,
) -> Result<RetryActionResult, &'static str> {
    let id = job_id(&input.job_id)?;
    let result: RequeuePayload = command(
        recovery_document(input.kind),
        json!({ "jobId": id, "reason": input.reason.trim() }),
        token,
        tenant,
    )
    .await?;
    result.into_result(id)
}

#[cfg(test)]
mod tests {
    use crate::model::ReplayActionStatus;

    use super::*;

    #[test]
    fn retry_scheduled_uses_the_action_dto_without_claiming_completion() {
        let response = serde_json::from_value::<Response<ReplayActionResult>>(json!({
            "result": {
                "status": "RETRY_SCHEDULED", "job_id": null, "retry_after_seconds": 5,
                "pages_processed": 1, "mutations_applied": 12
            }
        }))
        .unwrap();
        assert_eq!(response.result.status, ReplayActionStatus::RetryScheduled);
        assert_eq!(response.result.retry_after_seconds, Some(5));
        assert_eq!(response.result.mutations_applied, 12);
    }

    #[test]
    fn retry_selects_one_exact_kind_and_requires_recovery_evidence() {
        assert_eq!(recovery_document(RetryJobKind::Replay), REQUEUE_REPLAY);
        assert_eq!(
            recovery_document(RetryJobKind::Reconciliation),
            REQUEUE_RECONCILIATION
        );
        let id = Uuid::new_v4();
        let incomplete: RequeuePayload =
            serde_json::from_value(json!({ "status": "REQUEUED", "retryEpoch": null })).unwrap();
        assert_eq!(incomplete.into_result(id), Err(UNKNOWN_RESULT));
        let denied: RequeuePayload =
            serde_json::from_value(json!({ "status": "NOT_FOUND", "retryEpoch": null })).unwrap();
        assert_eq!(
            denied.into_result(id).unwrap().outcome,
            RetryActionStatus::NotFound
        );
    }

    #[test]
    fn uncontrolled_graphql_error_does_not_leak_or_imply_no_write() {
        let message = command_error(GraphqlHttpError::Graphql("private string".into()).into());
        assert_eq!(message, UNKNOWN_RESULT);
    }
}
