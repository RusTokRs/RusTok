use leptos::prelude::ServerFnError;
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

const INVENTORY_ADMIN_CLIENT_OWNER: &str = "rustok_inventory.admin";
const INVENTORY_ADMIN_CLIENT_BOUNDARY: &str = "inventory_admin_client_transport";
const INVENTORY_ADMIN_CLIENT_PUBLIC_MESSAGE: &str =
    "Inventory admin request could not be completed";

#[derive(Debug, Clone)]
pub enum InventoryTransportError {
    ServerFn,
}

impl Display for InventoryTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn => write!(f, "{INVENTORY_ADMIN_CLIENT_PUBLIC_MESSAGE}"),
        }
    }
}

impl std::error::Error for InventoryTransportError {}

pub(super) struct InventoryTransportErrorContext {
    operation: &'static str,
    correlation_id: String,
    tenant_id_length: Option<usize>,
    subject_id_length: Option<usize>,
    locale_length: Option<usize>,
    search_length: Option<usize>,
    status_length: Option<usize>,
    numeric_input_present: bool,
}

impl InventoryTransportErrorContext {
    pub(super) fn for_bootstrap() -> Self {
        Self::new("fetch_bootstrap")
    }

    pub(super) fn for_products(
        tenant_id: &str,
        locale: Option<&str>,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_products");
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = text_length(locale);
        context.search_length = text_length(search);
        context.status_length = text_length(status);
        context
    }

    pub(super) fn for_product(tenant_id: &str, product_id: &str, locale: Option<&str>) -> Self {
        let mut context = Self::new("fetch_product");
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.subject_id_length = Some(product_id.chars().count());
        context.locale_length = text_length(locale);
        context
    }

    pub(super) fn for_set_variant_quantity(tenant_id: &str, variant_id: &str) -> Self {
        Self::for_variant_input("set_variant_quantity", tenant_id, variant_id)
    }

    pub(super) fn for_adjust_variant_quantity(tenant_id: &str, variant_id: &str) -> Self {
        Self::for_variant_input("adjust_variant_quantity", tenant_id, variant_id)
    }

    pub(super) fn for_reserve_variant_quantity(tenant_id: &str, variant_id: &str) -> Self {
        Self::for_variant_input("reserve_variant_quantity", tenant_id, variant_id)
    }

    pub(super) fn for_check_variant_availability(tenant_id: &str, variant_id: &str) -> Self {
        Self::for_variant_input("check_variant_availability", tenant_id, variant_id)
    }

    pub(super) fn for_release_reservation_quantity(tenant_id: &str, variant_id: &str) -> Self {
        Self::for_variant_input("release_reservation_quantity", tenant_id, variant_id)
    }

    fn for_variant_input(operation: &'static str, tenant_id: &str, variant_id: &str) -> Self {
        let mut context = Self::new(operation);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.subject_id_length = Some(variant_id.chars().count());
        context.numeric_input_present = true;
        context
    }

    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            correlation_id: inventory_admin_client_correlation_id(operation),
            tenant_id_length: None,
            subject_id_length: None,
            locale_length: None,
            search_length: None,
            status_length: None,
            numeric_input_present: false,
        }
    }

    pub(super) fn map_error(&self, error: ServerFnError) -> InventoryTransportError {
        tracing::error!(
            raw_error = ?error,
            owner = INVENTORY_ADMIN_CLIENT_OWNER,
            owner_operation = self.operation,
            correlation_id = %self.correlation_id,
            tenant_id_present = self.tenant_id_length.is_some(),
            tenant_id_length = ?self.tenant_id_length,
            subject_id_present = self.subject_id_length.is_some(),
            subject_id_length = ?self.subject_id_length,
            locale_present = self.locale_length.is_some(),
            locale_length = ?self.locale_length,
            search_present = self.search_length.is_some(),
            search_length = ?self.search_length,
            status_present = self.status_length.is_some(),
            status_length = ?self.status_length,
            numeric_input_present = self.numeric_input_present,
            code = "inventory.admin_client_transport_failed",
            boundary = INVENTORY_ADMIN_CLIENT_BOUNDARY,
            "inventory admin client transport request failed"
        );

        InventoryTransportError::ServerFn
    }
}

fn inventory_admin_client_correlation_id(operation: &'static str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("inventory-admin-client:{operation}:{timestamp}")
}

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
}
