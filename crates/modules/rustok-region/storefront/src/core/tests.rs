use super::*;

fn sample_region() -> StorefrontRegion {
    StorefrontRegion {
        id: "eu".to_string(),
        name: "Europe".to_string(),
        currency_code: "EUR".to_string(),
        tax_provider_id: Some(" default ".to_string()),
        tax_rate: "20".to_string(),
        tax_included: true,
        country_tax_policies: vec![StorefrontRegionCountryTaxPolicy {
            country_code: "DE".to_string(),
            tax_rate: "19".to_string(),
            tax_included: false,
        }],
        countries: vec!["DE".to_string(), "FR".to_string()],
    }
}

#[test]
fn region_error_status_descriptors_are_host_visible_contract() {
    let descriptors = REGION_ERROR_STATUS_DESCRIPTORS;

    assert_eq!(descriptors.len(), 2);
    assert_eq!(
        descriptors.map(|descriptor| descriptor.stable_code),
        ["native_unavailable", "graphql_unavailable"]
    );
    assert_eq!(
        descriptors.map(|descriptor| descriptor.locale_key),
        [
            "region.error.status.nativeUnavailable",
            "region.error.status.graphqlUnavailable",
        ]
    );
    assert_eq!(
        region_error_status_descriptor(RegionErrorStatusCode::NativeUnavailable).stable_code,
        RegionErrorStatusCode::NativeUnavailable.as_str()
    );
}

#[test]
fn region_error_dom_evidence_uses_stable_attribute_contract() {
    let view_model = region_error_view_model(
        RegionErrorEvidence {
            failed_path: RegionStorefrontErrorPath::Graphql,
            fallback_attempted: true,
            native_error: Some("native failed".to_string()),
            graphql_error: Some("graphql failed".to_string()),
        },
        RegionErrorCopy {
            title: "Failed to load".to_string(),
            native_unavailable_body: "Native path is unavailable.".to_string(),
            graphql_unavailable_body: "Native and GraphQL paths are unavailable.".to_string(),
            native_status_label: "Native unavailable".to_string(),
            graphql_status_label: "GraphQL unavailable".to_string(),
            native_label: "native".to_string(),
            graphql_label: "graphql".to_string(),
        },
    );
    let evidence = region_error_dom_evidence(&view_model);

    assert_eq!(evidence.status_attribute, REGION_ERROR_STATUS_DOM_ATTRIBUTE);
    assert_eq!(evidence.status_value, "graphql_unavailable");
    assert_eq!(
        evidence.locale_key_attribute,
        REGION_ERROR_LOCALE_KEY_DOM_ATTRIBUTE
    );
    assert_eq!(
        evidence.locale_key_value,
        "region.error.status.graphqlUnavailable"
    );
}

#[test]
fn region_error_view_model_preserves_fallback_evidence_for_ui() {
    let view_model = region_error_view_model(
        RegionErrorEvidence {
            failed_path: RegionStorefrontErrorPath::Graphql,
            fallback_attempted: true,
            native_error: Some("tenant context missing".to_string()),
            graphql_error: Some("network unavailable".to_string()),
        },
        RegionErrorCopy {
            title: "Failed to load".to_string(),
            native_unavailable_body: "Native path is unavailable.".to_string(),
            graphql_unavailable_body: "Native and GraphQL paths are unavailable.".to_string(),
            native_status_label: "Native unavailable".to_string(),
            graphql_status_label: "GraphQL unavailable".to_string(),
            native_label: "native".to_string(),
            graphql_label: "graphql".to_string(),
        },
    );

    assert_eq!(
        view_model.status_code,
        RegionErrorStatusCode::GraphqlUnavailable
    );
    assert_eq!(view_model.status_code.as_str(), "graphql_unavailable");
    assert_eq!(
        view_model.status_locale_key,
        "region.error.status.graphqlUnavailable"
    );
    assert_eq!(view_model.status_label, "GraphQL unavailable");
    assert_eq!(view_model.title, "Failed to load");
    assert_eq!(view_model.body, "Native and GraphQL paths are unavailable.");
    assert_eq!(
        view_model.technical_detail.as_deref(),
        Some("native: tenant context missing | graphql: network unavailable")
    );
}

#[test]
fn region_error_view_model_classifies_native_only_error() {
    let view_model = region_error_view_model(
        RegionErrorEvidence {
            failed_path: RegionStorefrontErrorPath::NativeServer,
            fallback_attempted: false,
            native_error: Some("ssr required".to_string()),
            graphql_error: None,
        },
        RegionErrorCopy {
            title: "Failed to load".to_string(),
            native_unavailable_body: "Native path is unavailable.".to_string(),
            graphql_unavailable_body: "Native and GraphQL paths are unavailable.".to_string(),
            native_status_label: "Native unavailable".to_string(),
            graphql_status_label: "GraphQL unavailable".to_string(),
            native_label: "native".to_string(),
            graphql_label: "graphql".to_string(),
        },
    );

    assert_eq!(
        view_model.status_code,
        RegionErrorStatusCode::NativeUnavailable
    );
    assert_eq!(view_model.status_code.as_str(), "native_unavailable");
    assert_eq!(
        view_model.status_locale_key,
        "region.error.status.nativeUnavailable"
    );
    assert_eq!(view_model.status_label, "Native unavailable");
    assert_eq!(view_model.body, "Native path is unavailable.");
    assert_eq!(
        view_model.technical_detail.as_deref(),
        Some("native: ssr required")
    );
}

#[test]
fn resolve_storefront_regions_normalizes_selection_and_defaults_to_first_region() {
    let regions = vec![sample_region()];
    let data = resolve_storefront_regions(regions.clone(), Some(" eu ".to_string()));

    assert_eq!(data.selected_region_id.as_deref(), Some("eu"));
    assert_eq!(
        data.selected_region
            .as_ref()
            .map(|region| region.id.as_str()),
        Some("eu")
    );

    let defaulted = resolve_storefront_regions(regions, Some("   ".to_string()));
    assert_eq!(defaulted.selected_region_id.as_deref(), Some("eu"));
    assert_eq!(
        defaulted
            .selected_region
            .as_ref()
            .map(|region| region.id.as_str()),
        Some("eu")
    );
}

#[test]
fn resolve_storefront_regions_keeps_missing_selection_visible_without_selecting_item() {
    let data = resolve_storefront_regions(vec![sample_region()], Some("missing".to_string()));

    assert_eq!(data.selected_region_id.as_deref(), Some("missing"));
    assert!(data.selected_region.is_none());
}

#[test]
fn region_route_state_normalizes_host_route_query_contract() {
    let route_state = region_route_state(Some(" storefront "), Some(" eu ".to_string()));

    assert_eq!(route_state.route_segment, "storefront");
    assert_eq!(route_state.selected_region_id.as_deref(), Some("eu"));
    assert_eq!(
        route_state.selected_region_query_key,
        SELECTED_REGION_QUERY_KEY
    );

    let replace_update = selected_region_query_update(" eu ");
    assert_eq!(replace_update.query_key, SELECTED_REGION_QUERY_KEY);
    assert_eq!(
        replace_update.update.into_query_value().as_deref(),
        Some("eu")
    );

    let clear_update = selected_region_query_update("   ");
    assert_eq!(clear_update.query_key, SELECTED_REGION_QUERY_KEY);
    assert_eq!(clear_update.update.into_query_value(), None);

    assert_eq!(
        region_href("/storefront/regions", " eu "),
        "/storefront/regions?region=eu"
    );
    assert_eq!(
        region_href("/storefront/regions", "   "),
        "/storefront/regions"
    );
}

#[test]
fn route_segment_falls_back_without_ui_runtime() {
    assert_eq!(route_segment_or_default(Some(" storefront ")), "storefront");
    assert_eq!(route_segment_or_default(Some("   ")), DEFAULT_ROUTE_SEGMENT);
    assert_eq!(route_segment_or_default(None), DEFAULT_ROUTE_SEGMENT);
}

#[test]
fn rail_view_model_formats_route_and_region_summaries() {
    let region = sample_region();
    let view_model = rail_item_view_model(
        "/storefront/regions",
        &region,
        "tax included",
        "tax excluded",
        "none",
        "tax rate",
        "tax provider",
    );

    assert_eq!(view_model.name, "Europe");
    assert_eq!(view_model.href, "/storefront/regions?region=eu");
    assert_eq!(
        view_model.query_key_attribute,
        REGION_ROUTE_QUERY_KEY_DOM_ATTRIBUTE
    );
    assert_eq!(
        view_model.query_value_attribute,
        REGION_ROUTE_QUERY_VALUE_DOM_ATTRIBUTE
    );
    assert_eq!(view_model.query_key, SELECTED_REGION_QUERY_KEY);
    assert_eq!(view_model.query_value.as_deref(), Some("eu"));
    assert_eq!(view_model.tax_mode_label, "tax included");
    assert_eq!(view_model.country_summary, "EUR | DE, FR");
    assert_eq!(view_model.tax_summary, "20 tax rate | tax provider default");
    assert_eq!(view_model.tax_provider_id, "default");
}

#[test]
fn region_rail_view_model_collects_render_ready_list_state() {
    let regions = vec![sample_region()];
    let view_model = region_rail_view_model(
        "/storefront/regions",
        &regions,
        1,
        RegionRailLabels {
            title: "Available regions".to_string(),
            total_template: "{count} total".to_string(),
            empty_message: "No regions".to_string(),
            open_label: "Open".to_string(),
            tax_included_label: "tax included".to_string(),
            tax_excluded_label: "tax excluded".to_string(),
            empty_countries_label: "none".to_string(),
            tax_rate_label: "tax rate".to_string(),
            tax_provider_label: "tax provider".to_string(),
        },
    );

    assert_eq!(view_model.title, "Available regions");
    assert_eq!(view_model.total_label, "1 total");
    assert_eq!(view_model.empty_message, "No regions");
    assert_eq!(view_model.open_label, "Open");
    assert_eq!(view_model.items.len(), 1);
    assert_eq!(view_model.items[0].name, "Europe");
    assert_eq!(view_model.items[0].href, "/storefront/regions?region=eu");
    assert_eq!(view_model.items[0].country_summary, "EUR | DE, FR");
}

#[test]
fn selected_region_card_view_model_collects_render_ready_sections() {
    let mut region = sample_region();
    region.tax_provider_id = Some(" ".to_string());

    let view_model = selected_region_card_view_model(
        &region,
        SelectedRegionCardCopy {
            tax_included_label: "tax included",
            tax_excluded_label: "tax excluded",
            countries_label: "countries",
            no_countries_label: "no countries",
            currency_title: "Currency".to_string(),
            tax_rate_title: "Tax rate".to_string(),
            tax_provider_title: "Provider".to_string(),
            coverage_title: "Coverage".to_string(),
            country_policy_count_title: "Country policies".to_string(),
        },
    );

    assert_eq!(view_model.name, "Europe");
    assert_eq!(view_model.currency_code, "EUR");
    assert_eq!(view_model.tax_mode_label, "tax included");
    assert_eq!(view_model.countries_count_label, "2 countries");
    assert_eq!(view_model.metrics[2].value, DEFAULT_TAX_PROVIDER_ID);
    assert_eq!(view_model.countries_summary, "DE, FR");
    assert_eq!(
        view_model.country_policy_summaries,
        vec!["DE | 19 | tax excluded".to_string()]
    );
}

#[test]
fn selected_metrics_use_stable_fallbacks() {
    let mut region = sample_region();
    region.tax_provider_id = Some(" ".to_string());
    let metrics = selected_region_metrics(
        &region,
        "Currency".to_string(),
        "Tax rate".to_string(),
        "Provider".to_string(),
        "Coverage".to_string(),
        "Country policies".to_string(),
    );

    assert_eq!(metrics[2].value, DEFAULT_TAX_PROVIDER_ID);
    assert_eq!(metrics[3].value, "2");
    assert_eq!(metrics[4].value, "1");
}

#[test]
fn country_policy_summary_uses_tax_mode_label() {
    let policy = StorefrontRegionCountryTaxPolicy {
        country_code: "FR".to_string(),
        tax_rate: "20".to_string(),
        tax_included: true,
    };

    assert_eq!(
        country_policy_summary(&policy, "included", "excluded"),
        "FR | 20 | included"
    );
}
