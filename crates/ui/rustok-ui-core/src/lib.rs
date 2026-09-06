/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod css;
pub mod route_selection;
pub mod ui;

pub use css::{css_background_accent_class, css_hex_accent_class, normalize_css_hex_color};
pub use route_selection::{
    AdminQueryDependency, AdminQueryKey, AdminRouteQuerySchema, admin_route_query_schema,
    is_legacy_admin_query_key, sanitize_admin_route_query,
};
pub use ui::{
    UiRouteContext, UiRouteQueryIntent, UiRouteQueryUpdate, UiRouteQueryWrite,
    normalize_optional_ui_text, normalize_required_ui_text, normalize_ui_text, parse_ui_csv,
    route_query_update_for_text, ui_busy_key, ui_busy_key_last_segment_matches,
    ui_busy_key_matches_action, ui_busy_key_with_id, ui_optional_busy_key_with_id,
    ui_scoped_busy_key,
};
