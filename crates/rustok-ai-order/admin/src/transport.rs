use crate::core::{ai_order_admin_panel, AiOrderAdminPanel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiOrderAdminBootstrap {
    pub panel: AiOrderAdminPanel,
    pub transport_profile: &'static str,
    pub fallback_paths: [AiOrderAdminTransportPath; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiOrderAdminTransportPath {
    ServerFn(&'static str),
    Graphql(&'static str),
}

pub fn ai_order_admin_transport_with_fallback() -> [AiOrderAdminTransportPath; 2] {
    [
        AiOrderAdminTransportPath::ServerFn("native_ai_order_admin_bootstrap"),
        AiOrderAdminTransportPath::Graphql("aiOrderAdminBootstrap"),
    ]
}

pub fn bootstrap_ai_order_admin() -> AiOrderAdminBootstrap {
    AiOrderAdminBootstrap {
        panel: ai_order_admin_panel(),
        transport_profile: "native_server_with_graphql_fallback",
        fallback_paths: ai_order_admin_transport_with_fallback(),
    }
}
