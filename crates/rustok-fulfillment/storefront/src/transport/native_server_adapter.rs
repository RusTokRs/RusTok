mod raw_adapter;

use super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

pub async fn select_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    raw_adapter::select_shipping_option_server(request).await
}
