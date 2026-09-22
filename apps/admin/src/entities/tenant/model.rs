use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub active: bool,
}
