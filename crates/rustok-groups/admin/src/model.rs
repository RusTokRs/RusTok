use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminFilters {
    pub page: u64,
    pub per_page: u64,
    pub search: Option<String>,
    pub include_non_public: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminListItem {
    pub id: String,
    pub handle: String,
    pub title: String,
    pub visibility: String,
    pub join_policy: String,
    pub status: String,
    pub member_count: u64,
    pub effective_locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminDirectory {
    pub items: Vec<GroupsAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
