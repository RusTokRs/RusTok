mod server_functions;

use super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

pub async fn select_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    server_functions::select_shipping_option_server(request).await
}
