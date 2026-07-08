mod server_functions;

use super::{CheckoutCompletion, CheckoutCompletionTransportError, CompleteCheckoutRequest};

pub async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    server_functions::complete_checkout_server(request).await
}
