use crate::core::*;

#[test]
fn admin_route_query_intent_trims_open_region_and_clears_blank_values() {
    assert_eq!(
        region_admin_route_query_intent(Some("  region-eu  ")),
        RegionAdminRouteQueryIntent::Open {
            region_id: "region-eu".to_string(),
        }
    );
    assert_eq!(
        region_admin_route_query_intent(Some("   ")),
        RegionAdminRouteQueryIntent::Clear
    );
    assert_eq!(
        region_admin_route_query_intent(None),
        RegionAdminRouteQueryIntent::Clear
    );
}

#[test]
fn admin_route_query_updates_encode_host_writer_policy() {
    assert_eq!(REGION_ADMIN_SELECTED_QUERY_KEY, "region_id");
    assert_eq!(
        region_admin_open_query_update("  region-eu  "),
        Some(RegionAdminRouteQueryUpdate::Push {
            key: "region_id",
            value: "region-eu".to_string(),
        })
    );
    assert_eq!(region_admin_open_query_update(" "), None);
    assert_eq!(
        region_admin_saved_query_update("region-us"),
        Some(RegionAdminRouteQueryUpdate::Replace {
            key: "region_id",
            value: "region-us".to_string(),
        })
    );
    assert_eq!(
        region_admin_new_query_update(),
        RegionAdminRouteQueryUpdate::Clear { key: "region_id" }
    );
    assert_eq!(
        RegionAdminRouteQueryUpdate::Push {
            key: REGION_ADMIN_SELECTED_QUERY_KEY,
            value: "region-eu".to_string(),
        }
        .into_write(),
        rustok_ui_core::UiRouteQueryWrite {
            updates: vec![("region_id", Some("region-eu".to_string()))],
            replace: false,
        }
    );
    assert_eq!(
        region_admin_new_query_update().into_write(),
        rustok_ui_core::UiRouteQueryWrite {
            updates: vec![("region_id", None)],
            replace: true,
        }
    );
}
