#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TenantRouteScope {
    TenantBound,
    GlobalOperator,
    SelfResolvingHandshake,
}

fn path_is_or_descendant(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(crate) fn tenant_route_scope(path: &str) -> TenantRouteScope {
    if path == "/api/graphql/ws" {
        return TenantRouteScope::SelfResolvingHandshake;
    }

    if matches!(path, "/metrics" | "/api/openapi.json" | "/api/openapi.yaml")
        || path == "/api/graphql/schema.graphql"
        || path_is_or_descendant(path, "/api/install")
        || path_is_or_descendant(path, "/catalog")
        || path_is_or_descendant(path, "/catalog")
        || path_is_or_descendant(path, "/health")
    {
        TenantRouteScope::GlobalOperator
    } else {
        TenantRouteScope::TenantBound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_policy_distinguishes_global_and_self_resolving_surfaces() {
        assert_eq!(
            tenant_route_scope("/metrics"),
            TenantRouteScope::GlobalOperator
        );
        assert_eq!(
            tenant_route_scope("/healthcare"),
            TenantRouteScope::TenantBound
        );
        assert_eq!(
            tenant_route_scope("/api/graphql/ws"),
            TenantRouteScope::SelfResolvingHandshake
        );
        assert_eq!(
            tenant_route_scope("/api/graphql"),
            TenantRouteScope::TenantBound
        );
        assert_eq!(
            tenant_route_scope("/catalog/modules"),
            TenantRouteScope::GlobalOperator
        );
        assert_eq!(
            tenant_route_scope("/v2/catalog/publish"),
            TenantRouteScope::TenantBound
        );
    }
}
