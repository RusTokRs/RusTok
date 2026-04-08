use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerAdminBootstrap {
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerList {
    pub items: Vec<CustomerListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerListItem {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub locale: Option<String>,
    pub user_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerDetail {
    pub customer: CustomerRecord,
    pub profile: Option<CustomerProfileRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerRecord {
    pub id: String,
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub full_name: String,
    pub phone: Option<String>,
    pub locale: Option<String>,
    pub metadata_pretty: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerProfileRecord {
    pub handle: String,
    pub display_name: String,
    pub preferred_locale: Option<String>,
    pub visibility: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerDraft {
    pub user_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub locale: String,
}
