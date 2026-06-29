use crate::transport::{bootstrap_ai_content_admin, AiContentAdminBootstrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiContentAdmin {
    pub bootstrap: AiContentAdminBootstrap,
}

impl AiContentAdmin {
    pub fn load() -> Self {
        Self {
            bootstrap: bootstrap_ai_content_admin(),
        }
    }
}
