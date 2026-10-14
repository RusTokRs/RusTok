#[cfg(feature = "ssr")]
use crate::model::*;

#[cfg(feature = "ssr")]
pub async fn fetch_overview(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<MessengerOverviewPayload, String> {
    Ok(crate::core::default_overview_payload())
}

#[cfg(feature = "ssr")]
pub async fn fetch_channels(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<Vec<ChannelListItemPayload>, String> {
    Ok(Vec::new())
}

#[cfg(feature = "ssr")]
pub async fn fetch_e2ee_devices(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<Vec<E2eeDeviceAuditPayload>, String> {
    Ok(Vec::new())
}
