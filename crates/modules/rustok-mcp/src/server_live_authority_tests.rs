use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use rustok_core::registry::ModuleRegistry;

use crate::{
    McpAccessContext, McpAccessPolicy, McpRuntimeBinding, McpSessionContext,
    runtime::McpAccessResolver,
};

use super::RusToKMcpServer;

struct CountingResolver(Arc<AtomicUsize>);

#[async_trait]
impl McpAccessResolver for CountingResolver {
    async fn resolve_runtime_binding(
        &self,
        _session: &McpSessionContext,
    ) -> anyhow::Result<McpRuntimeBinding> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(McpRuntimeBinding {
            access_context: McpAccessContext {
                identity: None,
                granted_permissions: vec!["modules:manage".to_string()],
                policy: McpAccessPolicy::default(),
            },
            tenant_id: Some("tenant".to_string()),
            client_id: Some("client".to_string()),
            token_id: Some("token".to_string()),
        })
    }
}

#[tokio::test]
async fn live_binding_resolver_can_refresh_binding_repeatedly() {
    let calls = Arc::new(AtomicUsize::new(0));
    let server = RusToKMcpServer::new(ModuleRegistry::new())
        .with_shared_access_resolver(Arc::new(CountingResolver(Arc::clone(&calls))));

    for _ in 0..2 {
        let binding = server
            .resolve_live_binding()
            .await
            .expect("resolve current MCP authority")
            .expect("live resolver configured");
        assert_eq!(
            binding.access_context.granted_permissions,
            vec!["modules:manage".to_string()]
        );
    }

    assert_eq!(calls.load(Ordering::Relaxed), 2);
}
