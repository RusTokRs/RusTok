use rustok_grid::{ColumnFilters, FilterValue};

use crate::core::{
    CustomerAdminDisplayLabels, CustomerAdminDraftInput, CustomerAdminErrorLabels,
    CustomerAdminPageLabels, CustomerAdminSubmitCommandError, build_customer_admin_submit_command,
    customer_admin_list_header_view_model, customer_admin_shell_view_model,
    customer_admin_submit_error_message, customer_admin_transport_error_message,
    customer_detail_form_snapshot, customer_detail_view_model, customer_grid_columns,
    customer_list_item_class, customer_list_item_view_model, customer_list_request,
    empty_customer_admin_form_snapshot, filter_customers,
};
use crate::model::{
    CurrentTenant, CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList,
    CustomerListItem, CustomerProfileRecord, CustomerRecord,
};
use crate::transport::ApiError;

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    for marker in [
        "## Public Surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api_doc.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn test_models_derive_traits() {
    let tenant = CurrentTenant {
        id: "t-1".into(),
        slug: "tenant-slug".into(),
        name: "Test Tenant".into(),
    };
    let tenant2 = tenant.clone();
    assert_eq!(tenant, tenant2);
    assert!(format!("{tenant:?}").contains("CurrentTenant"));

    let bootstrap = CustomerAdminBootstrap {
        current_tenant: tenant.clone(),
    };
    assert_eq!(bootstrap.clone(), bootstrap);
    assert!(format!("{bootstrap:?}").contains("CustomerAdminBootstrap"));

    let item = CustomerListItem {
        id: "c-1".into(),
        email: "alice@example.com".into(),
        full_name: "Alice Smith".into(),
        phone: Some("+123456789".into()),
        locale: Some("en".into()),
        user_id: Some("u-1".into()),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
    };
    assert_eq!(item.clone(), item);
    assert!(format!("{item:?}").contains("Alice Smith"));

    let list = CustomerList {
        items: vec![item.clone()],
        total: 1,
        page: 1,
        per_page: 24,
        has_next: false,
    };
    assert_eq!(list.clone(), list);
    assert_eq!(list.items.len(), 1);

    let record = CustomerRecord {
        id: "c-1".into(),
        tenant_id: "t-1".into(),
        user_id: Some("u-1".into()),
        email: "alice@example.com".into(),
        first_name: Some("Alice".into()),
        last_name: Some("Smith".into()),
        full_name: "Alice Smith".into(),
        phone: Some("+123456789".into()),
        locale: Some("en".into()),
        metadata_pretty: "{}".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
    };
    assert_eq!(record.clone(), record);

    let profile = CustomerProfileRecord {
        handle: "asmith".into(),
        display_name: "Alice Smith".into(),
        preferred_locale: Some("en".into()),
        visibility: "public".into(),
        tags: vec!["vip".into()],
    };
    assert_eq!(profile.clone(), profile);

    let detail = CustomerDetail {
        customer: record,
        profile: Some(profile),
    };
    assert_eq!(detail.clone(), detail);

    let draft = CustomerDraft {
        user_id: "u-1".into(),
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        phone: "+123456789".into(),
        locale: "en".into(),
    };
    assert_eq!(draft.clone(), draft);

    let err = ApiError::ServerFn;
    assert_eq!(err.clone(), err);
    assert_eq!(
        err.to_string(),
        "Customer admin request could not be completed"
    );
}

#[test]
fn test_submit_command_validation() {
    let empty_email_input = CustomerAdminDraftInput {
        editing_customer_id: None,
        user_id: "".into(),
        email: "   ".into(),
        first_name: "Bob".into(),
        last_name: "Jones".into(),
        phone: "".into(),
    };
    let res = build_customer_admin_submit_command(empty_email_input, Some("en".into()));
    assert_eq!(res, Err(CustomerAdminSubmitCommandError::EmailRequired));

    let valid_input = CustomerAdminDraftInput {
        editing_customer_id: Some("cust-123".into()),
        user_id: "usr-456".into(),
        email: "bob@example.com".into(),
        first_name: "Bob".into(),
        last_name: "Jones".into(),
        phone: "+987654".into(),
    };
    let no_locale = build_customer_admin_submit_command(valid_input.clone(), None);
    assert_eq!(
        no_locale,
        Err(CustomerAdminSubmitCommandError::LocaleUnavailable)
    );

    let ok_cmd = build_customer_admin_submit_command(valid_input, Some("ru".into())).unwrap();
    assert_eq!(ok_cmd.customer_id.as_deref(), Some("cust-123"));
    assert_eq!(ok_cmd.draft.email, "bob@example.com");
    assert_eq!(ok_cmd.draft.locale, "ru");
    assert_eq!(ok_cmd.draft.user_id, "usr-456");
}

#[test]
fn test_error_message_formatting() {
    let labels = CustomerAdminErrorLabels {
        email_required: "Email is required".into(),
        locale_unavailable: "Locale is unavailable".into(),
        load_customer: "Failed to load customer".into(),
        save_customer: "Failed to save customer".into(),
        load_customers: "Failed to load customers".into(),
    };

    assert_eq!(
        customer_admin_submit_error_message(
            CustomerAdminSubmitCommandError::EmailRequired,
            &labels
        ),
        "Email is required"
    );
    assert_eq!(
        customer_admin_submit_error_message(
            CustomerAdminSubmitCommandError::LocaleUnavailable,
            &labels
        ),
        "Locale is unavailable"
    );

    assert_eq!(
        customer_admin_transport_error_message("Save failed", ""),
        "Save failed"
    );
    assert_eq!(
        customer_admin_transport_error_message("Save failed", "network timeout"),
        "Save failed: network timeout"
    );
}

#[test]
fn test_form_snapshot_lifecycle() {
    let empty = empty_customer_admin_form_snapshot();
    assert!(empty.editing_customer_id.is_none());
    assert!(empty.selected_detail.is_none());
    assert!(empty.email.is_empty());

    let detail = CustomerDetail {
        customer: CustomerRecord {
            id: "cust-99".into(),
            tenant_id: "ten-1".into(),
            user_id: Some("usr-99".into()),
            email: "charlie@example.com".into(),
            first_name: Some("Charlie".into()),
            last_name: None,
            full_name: "Charlie".into(),
            phone: Some("+111222".into()),
            locale: Some("fr".into()),
            metadata_pretty: "{}".into(),
            created_at: "2026-03-01".into(),
            updated_at: "2026-03-02".into(),
        },
        profile: None,
    };

    let snapshot = customer_detail_form_snapshot(detail);
    assert_eq!(snapshot.editing_customer_id.as_deref(), Some("cust-99"));
    assert_eq!(snapshot.email, "charlie@example.com");
    assert_eq!(snapshot.first_name, "Charlie");
    assert_eq!(snapshot.last_name, "");
    assert_eq!(snapshot.phone, "+111222");
    assert_eq!(snapshot.user_id, "usr-99");
    assert!(snapshot.selected_detail.is_some());
}

#[test]
fn test_filter_customers_and_columns() {
    let items = vec![
        CustomerListItem {
            id: "1".into(),
            email: "alice@company.com".into(),
            full_name: "Alice Cooper".into(),
            phone: Some("+100".into()),
            locale: Some("en".into()),
            user_id: None,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        },
        CustomerListItem {
            id: "2".into(),
            email: "bob@other.org".into(),
            full_name: "Bob Dylan".into(),
            phone: Some("+200".into()),
            locale: Some("ru".into()),
            user_id: Some("u2".into()),
            created_at: "2026-01-02".into(),
            updated_at: "2026-01-02".into(),
        },
    ];

    // Empty filters return all items
    let empty_filters = ColumnFilters::new();
    let all = filter_customers(&items, &empty_filters);
    assert_eq!(all.len(), 2);

    // Filter by name
    let mut name_filters = ColumnFilters::new();
    name_filters.set("name", FilterValue::Text("alice".into()));
    let by_name = filter_customers(&items, &name_filters);
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].id, "1");

    // Filter by account (linked vs standalone)
    let mut account_filters = ColumnFilters::new();
    account_filters.set("account", FilterValue::Select("linked".into()));
    let linked = filter_customers(&items, &account_filters);
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].id, "2");

    // Grid columns definition check
    let cols = customer_grid_columns(Some("en"));
    assert!(!cols.is_empty());
    assert!(cols[0].is_checkbox());
    assert_eq!(cols[1].id, "name");
}

#[test]
fn test_display_labels_and_view_models() {
    let labels = CustomerAdminDisplayLabels {
        linked_user: "linked user".into(),
        standalone_customer: "standalone customer".into(),
        not_linked_to_platform_user: "not linked".into(),
        no_phone: "no phone".into(),
        no_locale: "no locale".into(),
        no_tags: "no tags".into(),
        phone_label: "phone".into(),
        locale_label: "locale".into(),
        user_label: "user".into(),
        created_label: "created".into(),
        updated_label: "updated".into(),
        visibility_label: "visibility".into(),
    };

    let item = CustomerListItem {
        id: "c-1".into(),
        email: "alice@example.com".into(),
        full_name: "Alice".into(),
        phone: None,
        locale: None,
        user_id: None,
        created_at: "2026-01-01".into(),
        updated_at: "2026-01-02".into(),
    };
    let vm = customer_list_item_view_model(&item, &labels);
    assert_eq!(vm.linked_badge, "standalone customer");
    assert!(vm.meta.contains("no phone"));
    assert!(vm.meta.contains("no locale"));

    assert_eq!(
        customer_list_item_class(true),
        "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
    );
    assert_eq!(
        customer_list_item_class(false),
        "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
    );

    let detail = CustomerDetail {
        customer: CustomerRecord {
            id: "c-1".into(),
            tenant_id: "t-1".into(),
            user_id: None,
            email: "alice@example.com".into(),
            first_name: Some("Alice".into()),
            last_name: None,
            full_name: "Alice".into(),
            phone: None,
            locale: None,
            metadata_pretty: "{}".into(),
            created_at: "created".into(),
            updated_at: "updated".into(),
        },
        profile: None,
    };
    let detail_vm = customer_detail_view_model(&detail, &labels);
    assert_eq!(detail_vm.full_name, "Alice");
    assert_eq!(detail_vm.email, "alice@example.com");

    let page_labels = CustomerAdminPageLabels {
        badge: "customer".into(),
        title: "Customer Ops".into(),
        subtitle: "Operations subtitle".into(),
        list_title: "Customers".into(),
        list_subtitle_template: "Records for {tenant}".into(),
        list_subtitle_fallback: "Tenant records".into(),
        list_loading: "Loading...".into(),
        list_empty: "No items".into(),
        detail_title: "Detail".into(),
        detail_subtitle: "Detail sub".into(),
        detail_empty: "Select customer".into(),
        editor_subtitle: "Editor sub".into(),
        edit_title: "Edit".into(),
        create_title: "Create".into(),
        refresh_action: "Refresh".into(),
        open_action: "Open".into(),
        new_action: "New".into(),
        save_action: "Save".into(),
        create_action: "Create".into(),
        user_id_field: "User ID".into(),
        email_field: "Email".into(),
        first_name_field: "First name".into(),
        last_name_field: "Last name".into(),
        phone_field: "Phone".into(),
        customer_section: "Customer".into(),
        profile_section: "Profile".into(),
        metadata_section: "Metadata".into(),
        profile_empty: "No profile".into(),
    };

    let shell_vm = customer_admin_shell_view_model(&page_labels);
    assert_eq!(shell_vm.badge, "customer");
    assert_eq!(shell_vm.title, "Customer Ops");

    let header_named = customer_admin_list_header_view_model(Some("Alpha"), &page_labels);
    assert_eq!(header_named.subtitle, "Records for Alpha");

    let header_fallback = customer_admin_list_header_view_model(None, &page_labels);
    assert_eq!(header_fallback.subtitle, "Tenant records");

    let req = customer_list_request("test query");
    assert_eq!(req.search, "test query");
    assert_eq!(req.page, 1);
    assert_eq!(req.per_page, 24);
}
