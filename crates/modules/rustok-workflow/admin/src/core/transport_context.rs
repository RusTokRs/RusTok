#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAdminTransportContext {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
}

pub fn workflow_admin_transport_context(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> WorkflowAdminTransportContext {
    WorkflowAdminTransportContext { token, tenant_slug }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_admin_transport_context_preserves_host_scope() {
        let context = workflow_admin_transport_context(
            Some("token-1".to_string()),
            Some("tenant-a".to_string()),
        );

        assert_eq!(context.token.as_deref(), Some("token-1"));
        assert_eq!(context.tenant_slug.as_deref(), Some("tenant-a"));
    }
}
