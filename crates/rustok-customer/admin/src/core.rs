use crate::model::CustomerDraft;

pub const DEFAULT_CUSTOMER_PAGE: u64 = 1;
pub const DEFAULT_CUSTOMER_PER_PAGE: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerListRequest {
    pub search: String,
    pub page: u64,
    pub per_page: u64,
}

pub fn customer_list_request(search: impl Into<String>) -> CustomerListRequest {
    CustomerListRequest {
        search: search.into(),
        page: DEFAULT_CUSTOMER_PAGE,
        per_page: DEFAULT_CUSTOMER_PER_PAGE,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminDraftInput {
    pub editing_customer_id: Option<String>,
    pub user_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminSubmitCommand {
    pub customer_id: Option<String>,
    pub draft: CustomerDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomerAdminSubmitCommandError {
    EmailRequired,
    LocaleUnavailable,
}

pub fn build_customer_admin_submit_command(
    input: CustomerAdminDraftInput,
    effective_locale: Option<String>,
) -> Result<CustomerAdminSubmitCommand, CustomerAdminSubmitCommandError> {
    if input.email.trim().is_empty() {
        return Err(CustomerAdminSubmitCommandError::EmailRequired);
    }

    let Some(locale) = effective_locale else {
        return Err(CustomerAdminSubmitCommandError::LocaleUnavailable);
    };

    Ok(CustomerAdminSubmitCommand {
        customer_id: input.editing_customer_id,
        draft: CustomerDraft {
            user_id: input.user_id,
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            phone: input.phone,
            locale,
        },
    })
}

#[derive(Debug, Clone)]
pub struct CustomerAdminFormSnapshot {
    pub editing_customer_id: Option<String>,
    pub selected_detail: Option<crate::model::CustomerDetail>,
    pub user_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
}

pub fn empty_customer_admin_form_snapshot() -> CustomerAdminFormSnapshot {
    CustomerAdminFormSnapshot {
        editing_customer_id: None,
        selected_detail: None,
        user_id: String::new(),
        email: String::new(),
        first_name: String::new(),
        last_name: String::new(),
        phone: String::new(),
    }
}

pub fn customer_detail_form_snapshot(
    detail: crate::model::CustomerDetail,
) -> CustomerAdminFormSnapshot {
    CustomerAdminFormSnapshot {
        editing_customer_id: Some(detail.customer.id.clone()),
        user_id: detail.customer.user_id.clone().unwrap_or_default(),
        email: detail.customer.email.clone(),
        first_name: detail.customer.first_name.clone().unwrap_or_default(),
        last_name: detail.customer.last_name.clone().unwrap_or_default(),
        phone: detail.customer.phone.clone().unwrap_or_default(),
        selected_detail: Some(detail),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminDisplayLabels {
    pub linked_user: String,
    pub standalone_customer: String,
    pub not_linked_to_platform_user: String,
    pub no_phone: String,
    pub no_locale: String,
    pub no_tags: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerListItemViewModel {
    pub id: String,
    pub full_name: String,
    pub email: String,
    pub linked_badge: String,
    pub meta: String,
}

pub fn customer_list_item_view_model(
    customer: &crate::model::CustomerListItem,
    labels: &CustomerAdminDisplayLabels,
) -> CustomerListItemViewModel {
    let phone = customer
        .phone
        .clone()
        .unwrap_or_else(|| labels.no_phone.clone());
    let locale = customer
        .locale
        .clone()
        .unwrap_or_else(|| labels.no_locale.clone());
    let linked_badge = if customer.user_id.is_some() {
        labels.linked_user.clone()
    } else {
        labels.standalone_customer.clone()
    };

    CustomerListItemViewModel {
        id: customer.id.clone(),
        full_name: customer.full_name.clone(),
        email: customer.email.clone(),
        linked_badge,
        meta: format!(
            "phone: {phone} | locale: {locale} | updated {}",
            customer.updated_at
        ),
    }
}

pub fn customer_list_item_class(is_active: bool) -> &'static str {
    if is_active {
        "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
    } else {
        "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerProfileViewModel {
    pub identity: String,
    pub visibility: String,
    pub preferred_locale: String,
    pub tags: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerDetailViewModel {
    pub full_name: String,
    pub email: String,
    pub meta: String,
    pub created_at: String,
    pub updated_at: String,
    pub user_link: String,
    pub phone: String,
    pub locale: String,
    pub profile: Option<CustomerProfileViewModel>,
    pub metadata_pretty: String,
}

pub fn customer_detail_view_model(
    detail: &crate::model::CustomerDetail,
    labels: &CustomerAdminDisplayLabels,
) -> CustomerDetailViewModel {
    let user_link_for_meta = detail
        .customer
        .user_id
        .clone()
        .unwrap_or_else(|| labels.standalone_customer.clone());
    let locale = detail
        .customer
        .locale
        .clone()
        .unwrap_or_else(|| labels.no_locale.clone());
    let phone = detail
        .customer
        .phone
        .clone()
        .unwrap_or_else(|| labels.no_phone.clone());

    CustomerDetailViewModel {
        full_name: detail.customer.full_name.clone(),
        email: detail.customer.email.clone(),
        meta: format!("user: {user_link_for_meta} | locale: {locale}"),
        created_at: detail.customer.created_at.clone(),
        updated_at: detail.customer.updated_at.clone(),
        user_link: detail
            .customer
            .user_id
            .clone()
            .unwrap_or_else(|| labels.not_linked_to_platform_user.clone()),
        phone,
        locale,
        profile: detail
            .profile
            .as_ref()
            .map(|profile| CustomerProfileViewModel {
                identity: format!("{} @{}", profile.display_name, profile.handle),
                visibility: format!("visibility: {}", profile.visibility),
                preferred_locale: profile
                    .preferred_locale
                    .clone()
                    .unwrap_or_else(|| labels.no_locale.clone()),
                tags: if profile.tags.is_empty() {
                    labels.no_tags.clone()
                } else {
                    profile.tags.join(", ")
                },
            }),
        metadata_pretty: detail.customer.metadata_pretty.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminPageLabels {
    pub list_loading: String,
    pub list_empty: String,
    pub detail_empty: String,
    pub edit_title: String,
    pub create_title: String,
    pub save_action: String,
    pub create_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomerAdminListStateKind {
    Loading,
    Error,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminListStateViewModel {
    pub class: &'static str,
    pub message: String,
}

pub fn customer_admin_list_state_view_model(
    state: CustomerAdminListStateKind,
    labels: &CustomerAdminPageLabels,
    error_message: Option<&str>,
) -> CustomerAdminListStateViewModel {
    match state {
        CustomerAdminListStateKind::Loading => CustomerAdminListStateViewModel {
            class: "rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground",
            message: labels.list_loading.clone(),
        },
        CustomerAdminListStateKind::Error => CustomerAdminListStateViewModel {
            class: "rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive",
            message: error_message.unwrap_or_default().to_string(),
        },
        CustomerAdminListStateKind::Empty => CustomerAdminListStateViewModel {
            class: "rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground",
            message: labels.list_empty.clone(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminDetailEmptyViewModel {
    pub class: &'static str,
    pub message: String,
}

pub fn customer_admin_detail_empty_view_model(
    labels: &CustomerAdminPageLabels,
) -> CustomerAdminDetailEmptyViewModel {
    CustomerAdminDetailEmptyViewModel {
        class: "rounded-3xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground",
        message: labels.detail_empty.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminEditorViewModel {
    pub title: String,
    pub submit_label: String,
    pub user_id_disabled: bool,
    pub submit_disabled: bool,
    pub new_disabled: bool,
}

pub fn customer_admin_editor_view_model(
    is_editing: bool,
    is_busy: bool,
    labels: &CustomerAdminPageLabels,
) -> CustomerAdminEditorViewModel {
    CustomerAdminEditorViewModel {
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
        user_id_disabled: is_editing || is_busy,
        submit_disabled: is_busy,
        new_disabled: is_busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> CustomerAdminDisplayLabels {
        CustomerAdminDisplayLabels {
            linked_user: "linked user".to_string(),
            standalone_customer: "standalone customer".to_string(),
            not_linked_to_platform_user: "not linked to a platform user".to_string(),
            no_phone: "no phone".to_string(),
            no_locale: "no locale".to_string(),
            no_tags: "no tags".to_string(),
        }
    }

    fn page_labels() -> CustomerAdminPageLabels {
        CustomerAdminPageLabels {
            list_loading: "Loading customers...".to_string(),
            list_empty: "No customers match the current filters.".to_string(),
            detail_empty: "Open a customer to inspect the record.".to_string(),
            edit_title: "Edit Customer".to_string(),
            create_title: "Create Customer".to_string(),
            save_action: "Save customer".to_string(),
            create_action: "Create customer".to_string(),
        }
    }

    #[test]
    fn page_state_view_models_own_empty_error_and_editor_policy() {
        let labels = page_labels();

        let loading = customer_admin_list_state_view_model(
            CustomerAdminListStateKind::Loading,
            &labels,
            None,
        );
        assert_eq!(loading.message, "Loading customers...");
        assert!(loading.class.contains("border-dashed"));

        let error = customer_admin_list_state_view_model(
            CustomerAdminListStateKind::Error,
            &labels,
            Some("Failed to load customers: transport"),
        );
        assert_eq!(error.message, "Failed to load customers: transport");
        assert!(error.class.contains("destructive"));

        let empty =
            customer_admin_list_state_view_model(CustomerAdminListStateKind::Empty, &labels, None);
        assert_eq!(empty.message, "No customers match the current filters.");
        assert!(empty.class.contains("text-center"));

        let detail_empty = customer_admin_detail_empty_view_model(&labels);
        assert_eq!(
            detail_empty.message,
            "Open a customer to inspect the record."
        );

        let edit_busy = customer_admin_editor_view_model(true, true, &labels);
        assert_eq!(edit_busy.title, "Edit Customer");
        assert_eq!(edit_busy.submit_label, "Save customer");
        assert!(edit_busy.user_id_disabled);
        assert!(edit_busy.submit_disabled);
        assert!(edit_busy.new_disabled);

        let create_idle = customer_admin_editor_view_model(false, false, &labels);
        assert_eq!(create_idle.title, "Create Customer");
        assert_eq!(create_idle.submit_label, "Create customer");
        assert!(!create_idle.user_id_disabled);
        assert!(!create_idle.submit_disabled);
        assert!(!create_idle.new_disabled);
    }

    #[test]
    fn list_item_view_model_owns_link_and_fallback_policy() {
        let customer = crate::model::CustomerListItem {
            id: "customer-1".to_string(),
            email: "alice@example.com".to_string(),
            full_name: "Alice Example".to_string(),
            phone: None,
            locale: None,
            user_id: None,
            created_at: "2026-06-01T00:00:00Z".to_string(),
            updated_at: "2026-06-02T00:00:00Z".to_string(),
        };

        let view_model = customer_list_item_view_model(&customer, &labels());

        assert_eq!(view_model.id, "customer-1");
        assert_eq!(view_model.full_name, "Alice Example");
        assert_eq!(view_model.email, "alice@example.com");
        assert_eq!(view_model.linked_badge, "standalone customer");
        assert_eq!(
            view_model.meta,
            "phone: no phone | locale: no locale | updated 2026-06-02T00:00:00Z"
        );
        assert!(customer_list_item_class(true).contains("border-primary"));
        assert!(customer_list_item_class(false).contains("hover:border-primary"));
    }

    #[test]
    fn detail_view_model_maps_profile_and_form_snapshot() {
        let detail = crate::model::CustomerDetail {
            customer: crate::model::CustomerRecord {
                id: "customer-1".to_string(),
                tenant_id: "tenant-1".to_string(),
                user_id: Some("user-1".to_string()),
                email: "alice@example.com".to_string(),
                first_name: Some("Alice".to_string()),
                last_name: Some("Example".to_string()),
                full_name: "Alice Example".to_string(),
                phone: Some("+10000000000".to_string()),
                locale: Some("en".to_string()),
                metadata_pretty: "{}".to_string(),
                created_at: "created".to_string(),
                updated_at: "updated".to_string(),
            },
            profile: Some(crate::model::CustomerProfileRecord {
                handle: "alice".to_string(),
                display_name: "Alice".to_string(),
                preferred_locale: None,
                visibility: "public".to_string(),
                tags: vec![],
            }),
        };

        let snapshot = customer_detail_form_snapshot(detail.clone());
        assert_eq!(snapshot.editing_customer_id.as_deref(), Some("customer-1"));
        assert_eq!(snapshot.user_id, "user-1");
        assert_eq!(snapshot.email, "alice@example.com");
        assert_eq!(snapshot.first_name, "Alice");
        assert_eq!(snapshot.last_name, "Example");
        assert_eq!(snapshot.phone, "+10000000000");
        assert!(snapshot.selected_detail.is_some());

        let view_model = customer_detail_view_model(&detail, &labels());
        assert_eq!(view_model.meta, "user: user-1 | locale: en");
        assert_eq!(view_model.user_link, "user-1");
        assert_eq!(view_model.phone, "+10000000000");
        assert_eq!(view_model.locale, "en");
        assert_eq!(view_model.metadata_pretty, "{}");
        let profile = view_model.profile.expect("profile view model");
        assert_eq!(profile.identity, "Alice @alice");
        assert_eq!(profile.visibility, "visibility: public");
        assert_eq!(profile.preferred_locale, "no locale");
        assert_eq!(profile.tags, "no tags");
    }

    #[test]
    fn customer_list_request_uses_admin_defaults() {
        let request = customer_list_request("alice");

        assert_eq!(request.search, "alice");
        assert_eq!(request.page, DEFAULT_CUSTOMER_PAGE);
        assert_eq!(request.per_page, DEFAULT_CUSTOMER_PER_PAGE);
    }

    #[test]
    fn submit_command_preserves_payload_and_update_target() {
        let command = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: Some("customer-1".to_string()),
                user_id: "user-1".to_string(),
                email: "alice@example.com".to_string(),
                first_name: "Alice".to_string(),
                last_name: "Example".to_string(),
                phone: "+10000000000".to_string(),
            },
            Some("en".to_string()),
        )
        .expect("valid command");

        assert_eq!(command.customer_id.as_deref(), Some("customer-1"));
        assert_eq!(command.draft.user_id, "user-1");
        assert_eq!(command.draft.email, "alice@example.com");
        assert_eq!(command.draft.first_name, "Alice");
        assert_eq!(command.draft.last_name, "Example");
        assert_eq!(command.draft.phone, "+10000000000");
        assert_eq!(command.draft.locale, "en");
    }

    #[test]
    fn submit_command_rejects_missing_email_before_locale() {
        let error = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: None,
                user_id: String::new(),
                email: "   ".to_string(),
                first_name: String::new(),
                last_name: String::new(),
                phone: String::new(),
            },
            None,
        )
        .expect_err("email policy should run first");

        assert_eq!(error, CustomerAdminSubmitCommandError::EmailRequired);
    }

    #[test]
    fn submit_command_requires_host_locale() {
        let error = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: None,
                user_id: String::new(),
                email: "alice@example.com".to_string(),
                first_name: String::new(),
                last_name: String::new(),
                phone: String::new(),
            },
            None,
        )
        .expect_err("host locale is required");

        assert_eq!(error, CustomerAdminSubmitCommandError::LocaleUnavailable);
    }
}
