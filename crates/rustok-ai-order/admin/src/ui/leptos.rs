use crate::transport::{bootstrap_ai_order_admin, AiOrderAdminBootstrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiOrderAdmin {
    pub bootstrap: AiOrderAdminBootstrap,
}

impl AiOrderAdmin {
    pub fn load() -> Self {
        Self {
            bootstrap: bootstrap_ai_order_admin(),
        }
    }
}
