use crate::model::RegionDetail;

use crate::core::*;

#[test]
fn region_draft_builder_normalizes_form_state_without_ui_runtime() {
    let draft = build_region_draft(RegionFormInput {
        name: "  Europe  ",
        locale: " en ",
        currency_code: " eur ",
        tax_provider_id: " default ",
        tax_rate: " 20.0 ",
        tax_included: true,
        country_tax_policies: " [] ",
        countries: " DE, FR ",
        metadata: " {} ",
    });

    assert_eq!(draft.name, "Europe");
    assert_eq!(draft.locale, "en");
    assert_eq!(draft.currency_code, "eur");
    assert_eq!(draft.tax_provider_id, "default");
    assert_eq!(draft.tax_rate, "20.0");
    assert!(draft.tax_included);
    assert_eq!(draft.country_tax_policies, "[]");
    assert_eq!(draft.countries, "DE, FR");
    assert_eq!(draft.metadata, "{}");
    assert_eq!(missing_required_region_field(&draft), None);
}

#[test]
fn required_region_fields_reject_blank_values() {
    let draft = build_region_draft(RegionFormInput {
        name: " ",
        locale: "en",
        currency_code: "EUR",
        tax_provider_id: "",
        tax_rate: "0",
        tax_included: false,
        country_tax_policies: "[]",
        countries: "DE",
        metadata: "{}",
    });

    assert_eq!(
        missing_required_region_field(&draft),
        Some(RegionRequiredField::Name)
    );
}

#[test]
fn required_region_field_message_maps_validation_copy_without_ui_runtime() {
    let labels = RegionRequiredFieldLabels {
        name: "Name is required.".to_string(),
        currency_code: "Currency code is required.".to_string(),
        countries: "At least one country code is required.".to_string(),
    };

    assert_eq!(
        region_required_field_message(RegionRequiredField::Name, &labels),
        "Name is required."
    );
    assert_eq!(
        region_required_field_message(RegionRequiredField::CurrencyCode, &labels),
        "Currency code is required."
    );
    assert_eq!(
        region_required_field_message(RegionRequiredField::Countries, &labels),
        "At least one country code is required."
    );
}

#[test]
fn admin_submit_and_transport_errors_map_copy_without_ui_runtime() {
    let submit_labels = RegionAdminSubmitErrorLabels {
        locale_unavailable: "Host locale is unavailable.".to_string(),
        required_fields: RegionRequiredFieldLabels {
            name: "Name is required.".to_string(),
            currency_code: "Currency code is required.".to_string(),
            countries: "At least one country code is required.".to_string(),
        },
    };
    let transport_labels = RegionAdminTransportErrorLabels {
        load_region_context: "Failed to load region".to_string(),
        save_region_context: "Failed to save region".to_string(),
    };

    assert_eq!(
        region_admin_submit_error_message(
            RegionAdminSubmitError::HostLocaleUnavailable,
            &submit_labels,
        ),
        "Host locale is unavailable."
    );
    assert_eq!(
        region_admin_submit_error_message(
            RegionAdminSubmitError::MissingRequiredField(RegionRequiredField::Countries),
            &submit_labels,
        ),
        "At least one country code is required."
    );
    assert_eq!(
        region_admin_load_region_error_message("timeout", &transport_labels),
        "Failed to load region: timeout"
    );
    assert_eq!(
        region_admin_save_region_error_message("validation failed", &transport_labels),
        "Failed to save region: validation failed"
    );
}

#[test]
fn admin_save_mode_normalizes_editing_id_without_ui_runtime() {
    assert_eq!(region_admin_save_mode(None), RegionAdminSaveMode::Create);
    assert_eq!(
        region_admin_save_mode(Some("   ")),
        RegionAdminSaveMode::Create
    );
    assert_eq!(
        region_admin_save_mode(Some(" region-eu ")),
        RegionAdminSaveMode::Update {
            region_id: "region-eu".to_string(),
        }
    );
}

#[test]
fn admin_submit_preparation_builds_update_command_without_ui_runtime() {
    let command = prepare_region_admin_submit(RegionAdminSubmitInput {
        editing_id: Some(" region-eu "),
        locale: Some(" en "),
        name: " Europe ",
        currency_code: " EUR ",
        tax_provider_id: " vat ",
        tax_rate: " 20.0 ",
        tax_included: true,
        country_tax_policies: " [] ",
        countries: " DE, FR ",
        metadata: " {} ",
    })
    .expect("valid submit input should build a command");

    assert_eq!(
        command.mode,
        RegionAdminSaveMode::Update {
            region_id: "region-eu".to_string(),
        }
    );
    assert_eq!(command.payload.locale, "en");
    assert_eq!(command.payload.name, "Europe");
    assert_eq!(command.payload.currency_code, "EUR");
    assert_eq!(command.payload.countries, "DE, FR");
}

#[test]
fn admin_submit_preparation_rejects_missing_locale_and_required_fields() {
    let missing_locale = prepare_region_admin_submit(RegionAdminSubmitInput {
        editing_id: None,
        locale: None,
        name: "Europe",
        currency_code: "EUR",
        tax_provider_id: "",
        tax_rate: "0",
        tax_included: false,
        country_tax_policies: "[]",
        countries: "DE",
        metadata: "{}",
    });
    assert_eq!(
        missing_locale,
        Err(RegionAdminSubmitError::HostLocaleUnavailable)
    );

    let missing_name = prepare_region_admin_submit(RegionAdminSubmitInput {
        editing_id: None,
        locale: Some("en"),
        name: " ",
        currency_code: "EUR",
        tax_provider_id: "",
        tax_rate: "0",
        tax_included: false,
        country_tax_policies: "[]",
        countries: "DE",
        metadata: "{}",
    });
    assert_eq!(
        missing_name,
        Err(RegionAdminSubmitError::MissingRequiredField(
            RegionRequiredField::Name
        ))
    );
}

#[test]
fn admin_editor_form_state_uses_core_owned_empty_defaults() {
    let state = RegionAdminEditorFormState::empty();

    assert_eq!(state.editing_id, None);
    assert_eq!(state.tax_rate, DEFAULT_TAX_RATE_INPUT);
    assert!(!state.tax_included);
    assert_eq!(
        state.country_tax_policies,
        DEFAULT_COUNTRY_TAX_POLICIES_INPUT
    );
    assert_eq!(state.metadata, DEFAULT_METADATA_INPUT);
}

#[test]
fn admin_editor_form_state_maps_loaded_detail_without_ui_runtime() {
    let detail = crate::model::RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            tax_rate: "20.0".to_string(),
            tax_included: true,
            country_tax_policies_pretty: "[{\"country\":\"DE\"}]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{\"tier\":\"eu\"}".to_string(),
            created_at: "2026-06-12".to_string(),
            updated_at: "2026-06-12".to_string(),
        },
    };

    let state = RegionAdminEditorFormState::from_detail(&detail);

    assert_eq!(state.editing_id.as_deref(), Some("region-eu"));
    assert_eq!(state.name, "Europe");
    assert_eq!(state.tax_provider_id, "vat");
    assert_eq!(state.countries, "DE, FR");
    assert_eq!(state.country_tax_policies, "[{\"country\":\"DE\"}]");
    assert_eq!(state.metadata, "{\"tier\":\"eu\"}");
}

#[test]
fn admin_editor_field_view_model_keeps_form_copy_outside_ui_runtime() {
    let labels = RegionAdminEditorFieldLabels {
        name_placeholder: "Region name".to_string(),
        currency_code_placeholder: "Currency code".to_string(),
        tax_provider_id_placeholder: "Tax provider ID (optional)".to_string(),
        tax_rate_placeholder: "Tax rate".to_string(),
        tax_included_label: "Prices already include tax".to_string(),
        country_tax_policies_placeholder: "Country tax policies JSON".to_string(),
        countries_placeholder: "Countries (BY, RU, KZ)".to_string(),
        metadata_placeholder: "Metadata JSON".to_string(),
    };

    let view_model = build_region_admin_editor_field_view_model(&labels);

    assert_eq!(view_model.name_placeholder, "Region name");
    assert_eq!(view_model.currency_code_placeholder, "Currency code");
    assert_eq!(
        view_model.tax_provider_id_placeholder,
        "Tax provider ID (optional)"
    );
    assert_eq!(view_model.tax_rate_placeholder, "Tax rate");
    assert_eq!(view_model.tax_included_label, "Prices already include tax");
    assert_eq!(
        view_model.country_tax_policies_placeholder,
        "Country tax policies JSON"
    );
    assert_eq!(view_model.countries_placeholder, "Countries (BY, RU, KZ)");
    assert_eq!(view_model.metadata_placeholder, "Metadata JSON");
}

#[test]
fn admin_save_success_view_model_prepares_form_refresh_and_route_update() {
    let detail = RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            tax_rate: "20".to_string(),
            tax_included: true,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-13T00:00:00Z".to_string(),
            updated_at: "2026-06-13T00:00:00Z".to_string(),
        },
    };

    let view_model = region_admin_save_success(detail);

    assert!(view_model.selected.is_some());
    assert_eq!(
        view_model.form_state.editing_id,
        Some("region-eu".to_string())
    );
    assert_eq!(view_model.form_state.countries, "DE, FR");
    assert_eq!(
        view_model.route_update,
        Some(RegionAdminRouteQueryUpdate::Replace {
            key: "region_id",
            value: "region-eu".to_string(),
        })
    );
    assert!(view_model.refresh_list);
}

#[test]
fn admin_open_detail_view_model_maps_success_and_error_without_ui_runtime() {
    let detail = RegionDetail {
        region: crate::model::RegionRecord {
            id: "region-eu".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            tax_rate: "20".to_string(),
            tax_included: true,
            country_tax_policies_pretty: "[]".to_string(),
            countries: vec!["DE".to_string(), "FR".to_string()],
            metadata_pretty: "{}".to_string(),
            created_at: "2026-06-13T00:00:00Z".to_string(),
            updated_at: "2026-06-13T00:00:00Z".to_string(),
        },
    };

    let success = region_admin_open_detail_success(detail);

    assert_eq!(success.error, None);
    assert_eq!(success.form_state.editing_id, Some("region-eu".to_string()));
    assert_eq!(success.form_state.countries, "DE, FR");
    assert!(success.selected.is_some());

    let transport_error_labels = RegionAdminTransportErrorLabels {
        load_region_context: "Failed to load region".to_string(),
        save_region_context: "Failed to save region".to_string(),
    };
    let error = region_admin_open_detail_error("timeout", &transport_error_labels);

    assert_eq!(error.selected, None);
    assert_eq!(error.form_state, RegionAdminEditorFormState::empty());
    assert_eq!(
        error.error,
        Some("Failed to load region: timeout".to_string())
    );
}

#[test]
fn admin_editor_view_model_selects_create_or_edit_copy_without_ui_runtime() {
    let labels = RegionAdminEditorLabels {
        create_title: "Create Region".to_string(),
        edit_title: "Edit Region".to_string(),
        create_action: "Create region".to_string(),
        save_action: "Save region".to_string(),
    };

    let create_view_model = build_region_admin_editor_view_model(None, &labels);
    assert!(!create_view_model.is_editing);
    assert_eq!(create_view_model.title, "Create Region");
    assert_eq!(create_view_model.submit_label, "Create region");

    let edit_view_model = build_region_admin_editor_view_model(Some("region-eu"), &labels);
    assert!(edit_view_model.is_editing);
    assert_eq!(edit_view_model.title, "Edit Region");
    assert_eq!(edit_view_model.submit_label, "Save region");

    let blank_view_model = build_region_admin_editor_view_model(Some("  "), &labels);
    assert!(!blank_view_model.is_editing);
    assert_eq!(blank_view_model.title, "Create Region");
    assert_eq!(blank_view_model.submit_label, "Create region");
}
