use crate::core::{ai_content_admin_panel, AiContentAdminPanel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiContentAdminBootstrap {
    pub panel: AiContentAdminPanel,
    pub transport_profile: &'static str,
    pub fallback_paths: [AiContentAdminTransportPath; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiContentAdminTransportPath {
    ServerFn(&'static str),
    Graphql(&'static str),
}

pub fn ai_content_admin_transport_with_fallback() -> [AiContentAdminTransportPath; 2] {
    [
        AiContentAdminTransportPath::ServerFn("native_ai_content_admin_bootstrap"),
        AiContentAdminTransportPath::Graphql("aiContentAdminBootstrap"),
    ]
}

pub fn bootstrap_ai_content_admin() -> AiContentAdminBootstrap {
    AiContentAdminBootstrap {
        panel: ai_content_admin_panel(),
        transport_profile: "native_server_with_graphql_fallback",
        fallback_paths: ai_content_admin_transport_with_fallback(),
    }
}
