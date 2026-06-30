mod raw_adapter;

use super::{
    PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionFetchRequest,
    PaymentTransportError, RefundSummary, RefundSummaryFetchRequest,
};

pub async fn fetch_refund_summary(
    request: RefundSummaryFetchRequest,
) -> Result<RefundSummary, PaymentTransportError> {
    raw_adapter::fetch_refund_summary_server(request).await
}

pub async fn fetch_payment_collection(
    request: PaymentCollectionFetchRequest,
) -> Result<Option<PaymentCollection>, PaymentTransportError> {
    raw_adapter::fetch_payment_collection_server(request).await
}

pub async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentTransportError> {
    raw_adapter::create_payment_collection_server(request).await
}
