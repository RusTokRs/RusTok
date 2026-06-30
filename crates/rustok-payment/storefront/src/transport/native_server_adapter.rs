mod raw_adapter;

use super::{PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionTransportError};

pub async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentCollectionTransportError> {
    raw_adapter::create_payment_collection_server(request).await
}
