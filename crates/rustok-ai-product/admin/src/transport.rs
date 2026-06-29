use crate::core::{ai_product_admin_panel, AiProductAdminPanel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProductAdminBootstrap {
    pub panel: AiProductAdminPanel,
    pub transport_profile: &'static str,
    pub fallback_paths: [AiProductAdminTransportPath; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProductAdminTransportPath {
    ServerFn(&'static str),
    Graphql(&'static str),
}

pub fn ai_product_admin_transport_with_fallback() -> [AiProductAdminTransportPath; 2] {
    [
        AiProductAdminTransportPath::ServerFn("native_ai_product_admin_bootstrap"),
        AiProductAdminTransportPath::Graphql("aiProductAdminBootstrap"),
    ]
}

pub fn bootstrap_ai_product_admin() -> AiProductAdminBootstrap {
    AiProductAdminBootstrap {
        panel: ai_product_admin_panel(),
        transport_profile: "native_server_with_graphql_fallback",
        fallback_paths: ai_product_admin_transport_with_fallback(),
    }
}
