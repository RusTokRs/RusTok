use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::ingest_provider_webhook,
        crate::controllers::get_provider_event,
        crate::controllers::list_dead_letters,
        crate::controllers::replay_dead_letter
    ),
    components(schemas(
        crate::controllers::PaymentWebhookIngressResponse,
        crate::controllers::PaymentProviderEventAdminResponse
    )),
    tags(
        (
            name = "payment-webhooks",
            description = "Signature-verified payment provider webhook ingress"
        ),
        (
            name = "payment-provider-events",
            description = "Tenant-scoped provider event inspection and dead-letter replay"
        )
    )
)]
pub struct PaymentApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PaymentApiDoc::openapi()
}
