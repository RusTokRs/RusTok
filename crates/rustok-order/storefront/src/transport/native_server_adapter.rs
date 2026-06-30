mod raw_adapter;

use super::{CheckoutCompletion, CheckoutCompletionTransportError, CompleteCheckoutRequest};

pub async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    raw_adapter::complete_checkout_server(request).await
}
