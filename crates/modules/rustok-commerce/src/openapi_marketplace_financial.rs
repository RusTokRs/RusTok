use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::marketplace_financial::list_financial_operator_review,
        crate::controllers::marketplace_financial::show_financial_operation,
        crate::controllers::marketplace_financial::retry_financial_operation,
        crate::controllers::marketplace_financial::list_paid_event_operator_review,
        crate::controllers::marketplace_financial::show_paid_event,
        crate::controllers::marketplace_financial::retry_paid_event,
        crate::controllers::marketplace_financial::run_recovery_sweep,
        crate::controllers::marketplace_reversal_financial::list_operator_review,
        crate::controllers::marketplace_reversal_financial::show_event,
        crate::controllers::marketplace_reversal_financial::retry_event,
        crate::controllers::marketplace_reversal_financial::run_recovery_sweep,
        crate::controllers::marketplace_reversal_financial::list_adaptation_failures_operator_review,
        crate::controllers::marketplace_reversal_financial::show_adaptation_failure,
        crate::controllers::marketplace_reversal_financial::retry_adaptation_failure,
    ),
    components(
        schemas(
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepInput,
            crate::controllers::marketplace_financial::MarketplaceFinancialOperationResponse,
            crate::controllers::marketplace_financial::MarketplacePaidEventResponse,
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepFailureResponse,
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepInput,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalEventResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalAdaptationFailureResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepFailureResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepResponse,
        )
    ),
    tags(
        (name = "admin-marketplace-financial", description = "Marketplace financial recovery and reconciliation endpoints")
    )
)]
pub struct MarketplaceFinancialApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    MarketplaceFinancialApiDoc::openapi()
}
