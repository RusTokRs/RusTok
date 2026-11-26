use crate::model::RegionAdminBootstrap;

use crate::core::*;

#[test]
fn admin_shell_view_model_keeps_header_copy_outside_ui_runtime() {
    let labels = RegionAdminShellLabels {
        badge: "region".to_string(),
        title: "Region Operations".to_string(),
        subtitle: "Module-owned region workspace".to_string(),
    };

    let view_model = build_region_admin_shell_view_model(&labels);

    assert_eq!(view_model.badge, "region");
    assert_eq!(view_model.title, "Region Operations");
    assert_eq!(view_model.subtitle, "Module-owned region workspace");
}

#[test]
fn admin_list_header_view_model_formats_tenant_subtitle_without_ui_runtime() {
    let labels = RegionAdminListHeaderLabels {
        title: "Regions".to_string(),
        subtitle_template: "Tenant {tenant} region policy owned by the region module.".to_string(),
        subtitle_fallback: "Tenant-scoped region policy owned by the region module.".to_string(),
        refresh_action: "Refresh".to_string(),
    };
    let bootstrap = RegionAdminBootstrap {
        current_tenant: crate::model::CurrentTenant {
            id: "tenant-1".to_string(),
            slug: "tenant".to_string(),
            name: "  Demo Tenant  ".to_string(),
        },
    };

    let view_model = build_region_admin_list_header_view_model(Some(&bootstrap), &labels);

    assert_eq!(view_model.title, "Regions");
    assert_eq!(
        view_model.subtitle,
        "Tenant Demo Tenant region policy owned by the region module."
    );
    assert_eq!(view_model.refresh_action, "Refresh");

    let fallback = build_region_admin_list_header_view_model(None, &labels);
    assert_eq!(
        fallback.subtitle,
        "Tenant-scoped region policy owned by the region module."
    );
}

#[test]
fn admin_policy_section_view_model_applies_tax_provider_fallback() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: None,
            tax_rate: "20.0".to_string(),
            tax_included: false,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-12".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };
    let labels = RegionAdminPolicyLabels {
        currency: "currency".to_string(),
        tax_provider: "tax provider".to_string(),
        tax_rate: "tax rate".to_string(),
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
    };

    let view_model = build_region_admin_policy_section_view_model(&detail, &labels);

    assert_eq!(region_admin_countries_summary(&detail), "DE, FR");
    assert_eq!(view_model.rows[0].text, "currency: EUR");
    assert_eq!(
        view_model.rows[1].text,
        format!("tax provider: {DEFAULT_TAX_PROVIDER_ID}")
    );
    assert_eq!(view_model.rows[2].text, "tax rate: 20.0");
    assert_eq!(view_model.rows[3].text, "tax excluded");
}

#[test]
fn admin_raw_sections_view_model_keeps_pretty_json_payloads() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: None,
            tax_rate: "20.0".to_string(),
            tax_included: false,
            country_tax_policies_pretty: "[{\"country\":\"DE\"}]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{\"tier\":\"eu\"}".to_string(),
            created_at: "2026-06-12".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };
    let labels = RegionAdminRawSectionLabels {
        country_tax_policies_title: "Country Tax Policies".to_string(),
        metadata_title: "Metadata".to_string(),
    };

    let view_model = build_region_admin_raw_sections_view_model(&detail, &labels);

    assert_eq!(
        view_model.country_tax_policies.title,
        "Country Tax Policies"
    );
    assert_eq!(
        view_model.country_tax_policies.body,
        "[{\"country\":\"DE\"}]"
    );
    assert_eq!(view_model.metadata.title, "Metadata");
    assert_eq!(view_model.metadata.body, "{\"tier\":\"eu\"}");
}

#[test]
fn admin_list_item_view_model_formats_policy_state_without_ui_runtime() {
    let region = crate::model::RegionListItem {
        id: "region-eu".to_string(),
        name: "Europe".to_string(),
        currency_code: "EUR".to_string(),
        tax_provider_id: Some("vat".to_string()),
        country_count: 2,
        tax_rate: "20.0".to_string(),
        tax_included: true,
        countries_preview: "DE, FR".to_string(),
        updated_at: "2026-06-12".to_string(),
    };
    let labels = RegionAdminListLabels {
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
        countries: "countries".to_string(),
        tax_rate: "tax rate".to_string(),
        updated: "updated".to_string(),
    };

    let view_model = build_region_admin_list_item_view_model(&region, &labels);

    assert_eq!(view_model.id, "region-eu");
    assert_eq!(view_model.badge_label, "tax included");
    assert_eq!(view_model.summary, "EUR | DE, FR");
    assert_eq!(
        view_model.meta,
        "2 countries | tax rate 20.0 | updated 2026-06-12"
    );
    assert_eq!(
        region_admin_list_item_class(true),
        "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
    );
}

#[test]
fn admin_list_state_view_model_maps_loading_error_empty_and_ready_states() {
    let state_labels = RegionAdminListStateLabels {
        loading: "Loading regions...".to_string(),
        empty: "No regions have been created for this tenant yet.".to_string(),
        load_error_context: "Failed to load regions".to_string(),
        open_action: "Open".to_string(),
    };
    let item_labels = RegionAdminListLabels {
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
        countries: "countries".to_string(),
        tax_rate: "tax rate".to_string(),
        updated: "updated".to_string(),
    };

    assert_eq!(
        build_region_admin_list_state_view_model(None, &state_labels, &item_labels),
        RegionAdminListStateViewModel::Loading {
            message: "Loading regions...".to_string()
        }
    );

    assert_eq!(
        build_region_admin_list_state_view_model(
            Some(Err("network unavailable".to_string())),
            &state_labels,
            &item_labels,
        ),
        RegionAdminListStateViewModel::Error {
            message: "Failed to load regions: network unavailable".to_string()
        }
    );

    let empty_list = crate::model::RegionList { items: vec![] };
    assert_eq!(
        build_region_admin_list_state_view_model(
            Some(Ok(&empty_list)),
            &state_labels,
            &item_labels,
        ),
        RegionAdminListStateViewModel::Empty {
            message: "No regions have been created for this tenant yet.".to_string()
        }
    );

    let ready_list = crate::model::RegionList {
        items: vec![crate::model::RegionListItem {
            id: "region-eu".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            country_count: 2,
            tax_rate: "20.0".to_string(),
            tax_included: true,
            countries_preview: "DE, FR".to_string(),
            updated_at: "2026-06-12".to_string(),
        }],
    };

    assert_eq!(
        build_region_admin_list_state_view_model(
            Some(Ok(&ready_list)),
            &state_labels,
            &item_labels,
        ),
        RegionAdminListStateViewModel::Ready {
            items: vec![RegionAdminListItemViewModel {
                id: "region-eu".to_string(),
                name: "Europe".to_string(),
                badge_label: "tax included".to_string(),
                summary: "EUR | DE, FR".to_string(),
                meta: "2 countries | tax rate 20.0 | updated 2026-06-12".to_string(),
            }],
            open_action: "Open".to_string(),
        }
    );
}

#[test]
fn admin_detail_meta_uses_core_owned_tax_mode_policy() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: None,
            tax_rate: "20.0".to_string(),
            tax_included: false,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-12".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };
    let labels = RegionAdminDetailLabels {
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
        countries: "countries".to_string(),
        tax_rate: "tax rate".to_string(),
    };

    assert_eq!(
        build_region_admin_detail_meta(&detail, &labels),
        "2 countries | tax rate 20.0 (tax excluded)"
    );
}

#[test]
fn admin_detail_header_view_model_formats_summary_and_timestamps() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            tax_rate: "20.0".to_string(),
            tax_included: true,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-11".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };
    let detail_labels = RegionAdminDetailLabels {
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
        countries: "countries".to_string(),
        tax_rate: "tax rate".to_string(),
    };
    let header_labels = RegionAdminDetailHeaderLabels {
        created: "created".to_string(),
        updated: "updated".to_string(),
    };

    let view_model =
        build_region_admin_detail_header_view_model(&detail, &detail_labels, &header_labels);

    assert_eq!(view_model.name, "Europe");
    assert_eq!(view_model.summary, "EUR | DE, FR");
    assert_eq!(
        view_model.meta,
        "2 countries | tax rate 20.0 (tax included)"
    );
    assert_eq!(view_model.created, "created 2026-06-11");
    assert_eq!(view_model.updated, "updated 2026-06-12");
}

#[test]
fn admin_detail_panel_view_model_maps_empty_and_ready_states() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            tax_rate: "20.0".to_string(),
            tax_included: true,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-11".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };
    let panel_labels = RegionAdminDetailPanelLabels {
        title: "Region Detail".to_string(),
        subtitle: "Inspect policy".to_string(),
        policy_title: "Policy Baseline".to_string(),
        countries_title: "Country Coverage".to_string(),
        empty: "Open a region".to_string(),
    };
    let detail_labels = RegionAdminDetailLabels {
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
        countries: "countries".to_string(),
        tax_rate: "tax rate".to_string(),
    };
    let header_labels = RegionAdminDetailHeaderLabels {
        created: "created".to_string(),
        updated: "updated".to_string(),
    };
    let policy_labels = RegionAdminPolicyLabels {
        currency: "currency".to_string(),
        tax_provider: "tax provider".to_string(),
        tax_rate: "tax rate".to_string(),
        tax_included: "tax included".to_string(),
        tax_excluded: "tax excluded".to_string(),
    };
    let raw_labels = RegionAdminRawSectionLabels {
        country_tax_policies_title: "Country Tax Policies".to_string(),
        metadata_title: "Metadata".to_string(),
    };

    assert_eq!(
        build_region_admin_detail_panel_view_model(
            None,
            &panel_labels,
            &detail_labels,
            &header_labels,
            &policy_labels,
            &raw_labels,
        ),
        RegionAdminDetailPanelViewModel::Empty {
            message: "Open a region".to_string(),
        }
    );

    let ready = build_region_admin_detail_panel_view_model(
        Some(&detail),
        &panel_labels,
        &detail_labels,
        &header_labels,
        &policy_labels,
        &raw_labels,
    );

    match ready {
        RegionAdminDetailPanelViewModel::Ready { detail } => {
            let RegionAdminDetailPanelReadyViewModel {
                title,
                subtitle,
                policy_title,
                countries_title,
                countries_summary,
                header,
                policy,
                raw_sections,
            } = *detail;
            assert_eq!(title, "Region Detail");
            assert_eq!(subtitle, "Inspect policy");
            assert_eq!(policy_title, "Policy Baseline");
            assert_eq!(countries_title, "Country Coverage");
            assert_eq!(countries_summary, "DE, FR");
            assert_eq!(header.name, "Europe");
            assert_eq!(policy.rows.len(), 4);
            assert_eq!(raw_sections.metadata.body, "{}");
        }
        RegionAdminDetailPanelViewModel::Empty { .. } => {
            panic!("expected ready detail panel view-model")
        }
    }
}
