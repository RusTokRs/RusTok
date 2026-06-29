use crate::transport::{bootstrap_ai_product_admin, AiProductAdminBootstrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProductAdmin {
    pub bootstrap: AiProductAdminBootstrap,
}

impl AiProductAdmin {
    pub fn load() -> Self {
        Self {
            bootstrap: bootstrap_ai_product_admin(),
        }
    }
}
