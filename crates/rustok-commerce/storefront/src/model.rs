use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCommerceData {
    pub effective_locale: String,
    pub tenant_slug: Option<String>,
    pub tenant_default_locale: String,
    pub channel_slug: Option<String>,
    pub channel_resolution_source: Option<String>,
}
