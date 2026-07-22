#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::time::Duration;

use crate::direct::{
    DirectExecutionRequest, DirectExecutionResult, DirectTaskHandler, direct_operator_port_context,
    explain_result, generate_product_attributes,
};
use crate::model::{AiProductAttributesTaskInput, DirectExecutionTarget, ToolTrace};
use crate::service::{AiHostRuntime, AiOperatorContext};
use crate::{AiError, AiResult};
use rustok_ai_product::{PRODUCT_ATTRIBUTES_TASK_SLUG, PRODUCT_ATTRIBUTES_TOOL_NAME};
use rustok_product::{ProductProjectionRequest, dto::ProductResponse};

const PRODUCT_CATALOG_READ_DEADLINE: Duration = Duration::from_secs(3);

async fn product_context(
    runtime: &AiHostRuntime,
    operator: &AiOperatorContext,
    locale: &str,
    product_id: uuid::Uuid,
) -> (Option<ProductResponse>, serde_json::Value) {
    let deadline_ms = PRODUCT_CATALOG_READ_DEADLINE.as_millis() as u64;
    let Some(port) = runtime.product_catalog_read_port() else {
        return (
            None,
            json!({
                "source": "degraded",
                "catalog_enrichment": "skipped",
                "errors": [{
                    "kind": "unavailable",
                    "code": "ai_product.catalog_read_port_unavailable",
                    "retryable": true,
                }],
                "deadline_ms": deadline_ms,
            }),
        );
    };
    let context = direct_operator_port_context(
        operator,
        locale,
        PRODUCT_ATTRIBUTES_TASK_SLUG,
        PRODUCT_CATALOG_READ_DEADLINE,
    );
    match tokio::time::timeout(
        PRODUCT_CATALOG_READ_DEADLINE,
        port.read_product_projection(
            context,
            ProductProjectionRequest {
                product_id,
                locale: Some(locale.to_string()),
                fallback_locale: None,
            },
        ),
    )
    .await
    {
        Ok(Ok(product)) => (
            Some(product),
            json!({
                "source": "owner_port",
                "catalog_enrichment": "applied",
                "errors": [],
                "deadline_ms": deadline_ms,
            }),
        ),
        Err(_) => (
            None,
            json!({
                "source": "degraded",
                "catalog_enrichment": "skipped",
                "errors": [{
                    "kind": "deadline_exceeded",
                    "code": "ai_product.catalog_read_port_deadline_exceeded",
                    "retryable": true,
                }],
                "deadline_ms": deadline_ms,
            }),
        ),
        Ok(Err(error)) => (
            None,
            json!({
                "source": "degraded",
                "catalog_enrichment": "skipped",
                "errors": [{
                    "kind": error.kind,
                    "code": error.code,
                    "retryable": error.retryable,
                }],
                "deadline_ms": deadline_ms,
            }),
        ),
    }
}

pub struct ProductAttributesHandler;

#[async_trait]
impl DirectTaskHandler for ProductAttributesHandler {
    fn task_slug(&self) -> &'static str {
        PRODUCT_ATTRIBUTES_TASK_SLUG
    }

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiProductAttributesTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let (product, product_context) = product_context(
            runtime,
            operator,
            request.resolved_locale.as_str(),
            input.product_id,
        )
        .await;
        let generated = generate_product_attributes(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
            product.as_ref(),
        )
        .await?;
        let operation_payload = serde_json::to_value(&generated).map_err(AiError::Json)?;
        let summary = format!(
            "Prepared {} suggested product attributes.",
            generated.flex_attributes.len()
        );
        let trace = ToolTrace {
            tool_name: PRODUCT_ATTRIBUTES_TOOL_NAME.to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;
        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Commerce,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "product_id": input.product_id,
                "suggested_attributes": operation_payload,
                "product_context": product_context,
                "review_required": true,
                "persistence": "none",
            }),
        })
    }
}
