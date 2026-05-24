#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use crate::direct::{
    explain_result, generate_product_attributes, DirectExecutionRequest, DirectExecutionResult,
    DirectTaskHandler,
};
use crate::model::{AiProductAttributesTaskInput, DirectExecutionTarget, ToolTrace};
use crate::service::AiOperatorContext;
use crate::{AiError, AiResult};
use loco_rs::app::AppContext;
use rustok_api::loco::transactional_event_bus_from_context;
use rustok_commerce::CatalogService;

pub struct ProductAttributesHandler;

#[async_trait]
impl DirectTaskHandler for ProductAttributesHandler {
    fn task_slug(&self) -> &'static str {
        "product_attributes"
    }

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiProductAttributesTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let catalog = CatalogService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(app_ctx),
        );
        let product = catalog
            .get_product(operator.tenant_id, input.product_id)
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;
        let generated = generate_product_attributes(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
            &product,
        )
        .await?;
        let operation_payload = serde_json::to_value(&generated).map_err(AiError::Json)?;
        let summary = format!(
            "Prepared {} suggested product attributes.",
            generated.flex_attributes.len()
        );
        let trace = ToolTrace {
            tool_name: "direct.commerce.product_attributes".to_string(),
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
            metadata: json!({"direct_task": request.task_slug,"requested_locale": request.requested_locale,"resolved_locale": request.resolved_locale,"product_id": input.product_id,"suggested_attributes": operation_payload,}),
        })
    }
}
