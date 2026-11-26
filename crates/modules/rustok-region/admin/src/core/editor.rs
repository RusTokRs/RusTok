use rustok_ui_core::normalize_ui_text;

use crate::model::{RegionDetail, RegionDraft};

use super::route::{RegionAdminRouteQueryUpdate, region_admin_saved_query_update};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionFormInput<'a> {
    pub name: &'a str,
    pub locale: &'a str,
    pub currency_code: &'a str,
    pub tax_provider_id: &'a str,
    pub tax_rate: &'a str,
    pub tax_included: bool,
    pub country_tax_policies: &'a str,
    pub countries: &'a str,
    pub metadata: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRequiredField {
    Name,
    CurrencyCode,
    Countries,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRequiredFieldLabels {
    pub name: String,
    pub currency_code: String,
    pub countries: String,
}

pub fn region_required_field_message(
    field: RegionRequiredField,
    labels: &RegionRequiredFieldLabels,
) -> String {
    match field {
        RegionRequiredField::Name => labels.name.clone(),
        RegionRequiredField::CurrencyCode => labels.currency_code.clone(),
        RegionRequiredField::Countries => labels.countries.clone(),
    }
}

fn ui_text_or_default(value: &str) -> String {
    normalize_ui_text(value).unwrap_or_default()
}

pub fn build_region_draft(input: RegionFormInput<'_>) -> RegionDraft {
    RegionDraft {
        name: ui_text_or_default(input.name),
        locale: ui_text_or_default(input.locale),
        currency_code: ui_text_or_default(input.currency_code),
        tax_provider_id: ui_text_or_default(input.tax_provider_id),
        tax_rate: ui_text_or_default(input.tax_rate),
        tax_included: input.tax_included,
        country_tax_policies: ui_text_or_default(input.country_tax_policies),
        countries: ui_text_or_default(input.countries),
        metadata: ui_text_or_default(input.metadata),
    }
}

pub fn missing_required_region_field(input: &RegionDraft) -> Option<RegionRequiredField> {
    if input.name.is_empty() {
        Some(RegionRequiredField::Name)
    } else if input.currency_code.is_empty() {
        Some(RegionRequiredField::CurrencyCode)
    } else if input.countries.is_empty() {
        Some(RegionRequiredField::Countries)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionAdminSaveMode {
    Create,
    Update { region_id: String },
}

pub fn region_admin_save_mode(editing_id: Option<&str>) -> RegionAdminSaveMode {
    normalize_ui_text(editing_id.unwrap_or_default())
        .map(|region_id| RegionAdminSaveMode::Update { region_id })
        .unwrap_or(RegionAdminSaveMode::Create)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionAdminSubmitInput<'a> {
    pub editing_id: Option<&'a str>,
    pub locale: Option<&'a str>,
    pub name: &'a str,
    pub currency_code: &'a str,
    pub tax_provider_id: &'a str,
    pub tax_rate: &'a str,
    pub tax_included: bool,
    pub country_tax_policies: &'a str,
    pub countries: &'a str,
    pub metadata: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminSubmitCommand {
    pub mode: RegionAdminSaveMode,
    pub payload: RegionDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAdminSubmitError {
    HostLocaleUnavailable,
    MissingRequiredField(RegionRequiredField),
}

pub fn prepare_region_admin_submit(
    input: RegionAdminSubmitInput<'_>,
) -> Result<RegionAdminSubmitCommand, RegionAdminSubmitError> {
    let locale = normalize_ui_text(input.locale.unwrap_or_default())
        .ok_or(RegionAdminSubmitError::HostLocaleUnavailable)?;
    let payload = build_region_draft(RegionFormInput {
        name: input.name,
        locale: locale.as_str(),
        currency_code: input.currency_code,
        tax_provider_id: input.tax_provider_id,
        tax_rate: input.tax_rate,
        tax_included: input.tax_included,
        country_tax_policies: input.country_tax_policies,
        countries: input.countries,
        metadata: input.metadata,
    });

    if let Some(field) = missing_required_region_field(&payload) {
        return Err(RegionAdminSubmitError::MissingRequiredField(field));
    }

    Ok(RegionAdminSubmitCommand {
        mode: region_admin_save_mode(input.editing_id),
        payload,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminSubmitErrorLabels {
    pub locale_unavailable: String,
    pub required_fields: RegionRequiredFieldLabels,
}

pub fn region_admin_submit_error_message(
    error: RegionAdminSubmitError,
    labels: &RegionAdminSubmitErrorLabels,
) -> String {
    match error {
        RegionAdminSubmitError::HostLocaleUnavailable => labels.locale_unavailable.clone(),
        RegionAdminSubmitError::MissingRequiredField(field) => {
            region_required_field_message(field, &labels.required_fields)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminTransportErrorLabels {
    pub load_region_context: String,
    pub save_region_context: String,
}

pub fn region_admin_load_region_error_message(
    error: &str,
    labels: &RegionAdminTransportErrorLabels,
) -> String {
    error_with_context(labels.load_region_context.as_str(), error)
}

pub fn region_admin_save_region_error_message(
    error: &str,
    labels: &RegionAdminTransportErrorLabels,
) -> String {
    error_with_context(labels.save_region_context.as_str(), error)
}

pub const DEFAULT_TAX_RATE_INPUT: &str = "0";
pub const DEFAULT_COUNTRY_TAX_POLICIES_INPUT: &str = "[]";
pub const DEFAULT_METADATA_INPUT: &str = "{}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFormState {
    pub editing_id: Option<String>,
    pub name: String,
    pub currency_code: String,
    pub tax_provider_id: String,
    pub tax_rate: String,
    pub tax_included: bool,
    pub country_tax_policies: String,
    pub countries: String,
    pub metadata: String,
}

impl RegionAdminEditorFormState {
    pub fn empty() -> Self {
        Self {
            editing_id: None,
            name: String::new(),
            currency_code: String::new(),
            tax_provider_id: String::new(),
            tax_rate: DEFAULT_TAX_RATE_INPUT.to_string(),
            tax_included: false,
            country_tax_policies: DEFAULT_COUNTRY_TAX_POLICIES_INPUT.to_string(),
            countries: String::new(),
            metadata: DEFAULT_METADATA_INPUT.to_string(),
        }
    }

    pub fn from_detail(detail: &RegionDetail) -> Self {
        Self {
            editing_id: Some(detail.region.id.clone()),
            name: detail.region.name.clone(),
            currency_code: detail.region.currency_code.clone(),
            tax_provider_id: detail.region.tax_provider_id.clone().unwrap_or_default(),
            tax_rate: detail.region.tax_rate.clone(),
            tax_included: detail.region.tax_included,
            country_tax_policies: detail.region.country_tax_policies_pretty.clone(),
            countries: detail.region.countries.join(", "),
            metadata: detail.region.metadata_pretty.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminSaveSuccessViewModel {
    pub selected: Option<RegionDetail>,
    pub form_state: RegionAdminEditorFormState,
    pub route_update: Option<RegionAdminRouteQueryUpdate>,
    pub refresh_list: bool,
}

pub fn region_admin_save_success(detail: RegionDetail) -> RegionAdminSaveSuccessViewModel {
    let route_update = region_admin_saved_query_update(detail.region.id.as_str());
    let form_state = RegionAdminEditorFormState::from_detail(&detail);

    RegionAdminSaveSuccessViewModel {
        selected: Some(detail),
        form_state,
        route_update,
        refresh_list: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminOpenDetailViewModel {
    pub selected: Option<RegionDetail>,
    pub form_state: RegionAdminEditorFormState,
    pub error: Option<String>,
}

pub fn region_admin_open_detail_success(detail: RegionDetail) -> RegionAdminOpenDetailViewModel {
    let form_state = RegionAdminEditorFormState::from_detail(&detail);

    RegionAdminOpenDetailViewModel {
        selected: Some(detail),
        form_state,
        error: None,
    }
}

pub fn region_admin_open_detail_error(
    error: &str,
    labels: &RegionAdminTransportErrorLabels,
) -> RegionAdminOpenDetailViewModel {
    RegionAdminOpenDetailViewModel {
        selected: None,
        form_state: RegionAdminEditorFormState::empty(),
        error: Some(region_admin_load_region_error_message(error, labels)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorLabels {
    pub create_title: String,
    pub edit_title: String,
    pub create_action: String,
    pub save_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorViewModel {
    pub is_editing: bool,
    pub title: String,
    pub submit_label: String,
}

pub fn build_region_admin_editor_view_model(
    editing_id: Option<&str>,
    labels: &RegionAdminEditorLabels,
) -> RegionAdminEditorViewModel {
    let is_editing = editing_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    RegionAdminEditorViewModel {
        is_editing,
        title: if is_editing {
            labels.edit_title.clone()
        } else {
            labels.create_title.clone()
        },
        submit_label: if is_editing {
            labels.save_action.clone()
        } else {
            labels.create_action.clone()
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFieldLabels {
    pub name_placeholder: String,
    pub currency_code_placeholder: String,
    pub tax_provider_id_placeholder: String,
    pub tax_rate_placeholder: String,
    pub tax_included_label: String,
    pub country_tax_policies_placeholder: String,
    pub countries_placeholder: String,
    pub metadata_placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFieldViewModel {
    pub name_placeholder: String,
    pub currency_code_placeholder: String,
    pub tax_provider_id_placeholder: String,
    pub tax_rate_placeholder: String,
    pub tax_included_label: String,
    pub country_tax_policies_placeholder: String,
    pub countries_placeholder: String,
    pub metadata_placeholder: String,
}

pub fn build_region_admin_editor_field_view_model(
    labels: &RegionAdminEditorFieldLabels,
) -> RegionAdminEditorFieldViewModel {
    RegionAdminEditorFieldViewModel {
        name_placeholder: labels.name_placeholder.clone(),
        currency_code_placeholder: labels.currency_code_placeholder.clone(),
        tax_provider_id_placeholder: labels.tax_provider_id_placeholder.clone(),
        tax_rate_placeholder: labels.tax_rate_placeholder.clone(),
        tax_included_label: labels.tax_included_label.clone(),
        country_tax_policies_placeholder: labels.country_tax_policies_placeholder.clone(),
        countries_placeholder: labels.countries_placeholder.clone(),
        metadata_placeholder: labels.metadata_placeholder.clone(),
    }
}

pub(super) fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}
