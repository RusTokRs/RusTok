#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::time::Duration;

use crate::direct::{
    DirectExecutionRequest, DirectExecutionResult, DirectExplanationRequest, DirectTaskHandler,
    direct_operator_port_context, explain_result, generate_product_attributes,
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
        let explanation = explain_result(DirectExplanationRequest {
            provider: &request.provider,
            provider_config: &request.provider_config,
            system_prompt: request.system_prompt.as_deref(),
            locale: request.resolved_locale.as_str(),
            assistant_prompt: input.assistant_prompt.as_deref(),
            summary: &summary,
            payload: &operation_payload,
            stream_emitter: request.stream_emitter.clone(),
        })
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

#[cfg(test)]
mod remote_profile_tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use rustok_api::{PortActor, PortContext, PortError};
    use rustok_core::ModuleRegistry;
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use rustok_product::{
        ProductCatalogReadPort, ProductCatalogReadProfile, ProductCatalogReadRuntime,
        ProductProjectionRequest, PublishedProductsRequest, StorefrontProductList,
        VariantProductProjectionRequest, dto::ProductResponse,
    };
    use rustok_product_transport::{
        GrpcProductCatalogReadProvider, ProductCatalogGrpcOperation, ProductCatalogGrpcService,
        TrustedProductCatalogAuthority,
        proto::product_catalog_read_service_server::ProductCatalogReadServiceServer,
    };
    use rustok_secrets::SecretResolverRegistry;
    use sea_orm::Database;
    use tokio::sync::oneshot;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::{Endpoint, Server};
    use uuid::Uuid;

    use super::product_context;
    use crate::engine::{AiProviderTargetCatalog, ProviderEgressPolicy};
    use crate::service::{AiHostRuntime, AiOperatorContext};

    #[derive(Clone, Copy)]
    enum RemoteFailure {
        Unavailable,
        Timeout,
    }

    impl RemoteFailure {
        fn port_error(self) -> PortError {
            match self {
                Self::Unavailable => PortError::unavailable(
                    "product.remote_unavailable",
                    "remote Product catalog is unavailable",
                ),
                Self::Timeout => PortError::timeout(
                    "product.remote_timeout",
                    "remote Product catalog deadline was exceeded",
                ),
            }
        }

        const fn expected_kind(self) -> &'static str {
            match self {
                Self::Unavailable => "unavailable",
                Self::Timeout => "timeout",
            }
        }

        const fn expected_code(self) -> &'static str {
            match self {
                Self::Unavailable => "product.remote_unavailable",
                Self::Timeout => "product.remote_timeout",
            }
        }
    }

    struct FailingProductCatalogReadPort {
        failure: RemoteFailure,
    }

    #[async_trait]
    impl ProductCatalogReadPort for FailingProductCatalogReadPort {
        async fn read_product_projection(
            &self,
            _context: PortContext,
            _request: ProductProjectionRequest,
        ) -> Result<ProductResponse, PortError> {
            Err(self.failure.port_error())
        }

        async fn read_variant_product_projection(
            &self,
            _context: PortContext,
            _request: VariantProductProjectionRequest,
        ) -> Result<ProductResponse, PortError> {
            Err(self.failure.port_error())
        }

        async fn list_published_products(
            &self,
            _context: PortContext,
            _request: PublishedProductsRequest,
        ) -> Result<StorefrontProductList, PortError> {
            Err(self.failure.port_error())
        }
    }

    async fn runtime_with_remote_failure(
        tenant_id: Uuid,
        failure: RemoteFailure,
    ) -> (
        AiHostRuntime,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback Product listener should bind");
        let address = listener
            .local_addr()
            .expect("loopback Product listener address should exist");
        let incoming = TcpListenerStream::new(listener);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let authority = TrustedProductCatalogAuthority::new(
            tenant_id.to_string(),
            PortActor::service("trusted-ai-remote-profile"),
        )
        .allow_operation(ProductCatalogGrpcOperation::ReadProductProjection);
        let service = ProductCatalogReadServiceServer::with_interceptor(
            ProductCatalogGrpcService::new(Arc::new(FailingProductCatalogReadPort { failure })),
            move |mut request: tonic::Request<()>| {
                request.extensions_mut().insert(authority.clone());
                Ok(request)
            },
        );
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("loopback Product gRPC server should run");
        });
        let provider = GrpcProductCatalogReadProvider::connect(
            Endpoint::from_shared(format!("http://{address}"))
                .expect("loopback Product endpoint should parse")
                .connect_timeout(Duration::from_secs(5)),
        )
        .await
        .expect("loopback Product gRPC client should connect");
        let product_runtime = ProductCatalogReadRuntime::external(Arc::new(provider));
        assert_eq!(
            product_runtime.profile(),
            ProductCatalogReadProfile::External
        );

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory AI database should connect");
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        let runtime = AiHostRuntime::new(
            db,
            event_bus,
            ModuleRegistry::default(),
            SecretResolverRegistry::builder().build(),
            ProviderEgressPolicy::default(),
            AiProviderTargetCatalog::default(),
        )
        .with_product_catalog_read_port(Some(product_runtime.read_port()));
        (runtime, shutdown_tx, server)
    }

    async fn assert_remote_failure_degrades_ai_enrichment(failure: RemoteFailure) {
        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let (runtime, shutdown_tx, server) = runtime_with_remote_failure(tenant_id, failure).await;
        let operator = AiOperatorContext {
            tenant_id,
            user_id: Uuid::new_v4(),
            permissions: Vec::new(),
            role_slugs: vec!["ai-operator".to_string()],
            preferred_locale: Some("en".to_string()),
        };

        let (product, metadata) = product_context(&runtime, &operator, "en", product_id).await;
        let _ = shutdown_tx.send(());
        server
            .await
            .expect("loopback Product gRPC server task should stop");

        assert!(product.is_none());
        assert_eq!(metadata["source"], "degraded");
        assert_eq!(metadata["catalog_enrichment"], "skipped");
        assert_eq!(metadata["errors"][0]["kind"], failure.expected_kind());
        assert_eq!(metadata["errors"][0]["code"], failure.expected_code());
        assert_eq!(metadata["errors"][0]["retryable"], true);
        assert_eq!(metadata["deadline_ms"], 3_000);
    }

    #[tokio::test]
    async fn remote_product_unavailable_degrades_ai_enrichment() {
        assert_remote_failure_degrades_ai_enrichment(RemoteFailure::Unavailable).await;
    }

    #[tokio::test]
    async fn remote_product_timeout_degrades_ai_enrichment() {
        assert_remote_failure_degrades_ai_enrichment(RemoteFailure::Timeout).await;
    }
}
