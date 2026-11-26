use rustok_ui_core::{AdminQueryKey, UiRouteQueryIntent, normalize_ui_text};

pub const REGION_ADMIN_SELECTED_QUERY_KEY: &str = AdminQueryKey::RegionId.as_str();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionAdminRouteQueryIntent {
    Open { region_id: String },
    Clear,
}

pub type RegionAdminRouteQueryUpdate = UiRouteQueryIntent;

pub fn region_admin_route_query_intent(
    selected_region_query: Option<&str>,
) -> RegionAdminRouteQueryIntent {
    normalize_ui_text(selected_region_query.unwrap_or_default())
        .map(|region_id| RegionAdminRouteQueryIntent::Open { region_id })
        .unwrap_or(RegionAdminRouteQueryIntent::Clear)
}

pub fn region_admin_open_query_update(region_id: &str) -> Option<RegionAdminRouteQueryUpdate> {
    normalize_ui_text(region_id)
        .map(|region_id| UiRouteQueryIntent::push(REGION_ADMIN_SELECTED_QUERY_KEY, region_id))
}

pub fn region_admin_saved_query_update(region_id: &str) -> Option<RegionAdminRouteQueryUpdate> {
    normalize_ui_text(region_id)
        .map(|region_id| UiRouteQueryIntent::replace(REGION_ADMIN_SELECTED_QUERY_KEY, region_id))
}

pub fn region_admin_new_query_update() -> RegionAdminRouteQueryUpdate {
    UiRouteQueryIntent::clear(REGION_ADMIN_SELECTED_QUERY_KEY)
}
