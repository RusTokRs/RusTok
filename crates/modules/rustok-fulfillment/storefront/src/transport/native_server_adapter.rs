mod native_client_error_safety;
mod server_functions;

use self::native_client_error_safety::NativeClientErrorContext;
use super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

pub async fn select_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    let context = NativeClientErrorContext::validate_and_new(&request)?;
    server_functions::select_shipping_option_server(request)
        .await
        .map_err(|error| context.map_error(error))
}
