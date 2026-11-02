mod core;
mod i18n;
mod model;
mod transport;

mod ui;

pub use ui::leptos::PricingView;

// Shared, framework-agnostic request/context admission used by Product's buyer adapter.
pub use core::{
    StorefrontPricingQueryError, normalize_pricing_context, parse_optional_currency_code,
    parse_optional_uuid_string, parse_pricing_route_quantity, parse_resolution_quantity,
    pricing_context_matches_request, valid_quote_time,
};
pub use model::PricingResolutionContext;
