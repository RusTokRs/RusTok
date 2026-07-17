from pathlib import Path

path = Path("apps/server/src/middleware/tenant_resolution.rs")
source = path.read_text()


def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    source = source.replace(old, new, 1)


route_policy = '''#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
        || path_is_or_descendant(path, "/v1/catalog")
        || path_is_or_descendant(path, "/catalog")
        || path_is_or_descendant(path, "/health")
    {
        TenantRouteScope::GlobalOperator
    } else {
        TenantRouteScope::TenantBound
    }
}

'''
replace_once(route_policy, "", "remove route policy from resolver")
replace_once(
    "    CompatibilitySlugHeader,\n    Host,",
    "    CompatibilitySlugHeader,\n    SelfResolvingHandshake,\n    Host,",
    "add self-resolving source",
)
replace_once(
    '            Self::CompatibilitySlugHeader => "compatibility_slug_header",\n            Self::Host => "host",',
    '            Self::CompatibilitySlugHeader => "compatibility_slug_header",\n            Self::SelfResolvingHandshake => "self_resolving_handshake",\n            Self::Host => "host",',
    "map self-resolving source",
)
replace_once(
    '''pub(crate) fn validated_slug_identifier(
    value: &str,
) -> Result<ResolvedTenantIdentifier, TenantResolutionError> {
    validate_slug(value).map(ResolvedTenantIdentifier::Slug)
}
''',
    '''pub(crate) fn resolve_explicit_slug(
    value: &str,
) -> Result<TenantResolution, TenantResolutionError> {
    Ok(TenantResolution {
        identifier: ResolvedTenantIdentifier::Slug(validate_slug(value)?),
        source: TenantResolutionSource::SelfResolvingHandshake,
        asserted_slug: None,
    })
}
''',
    "replace raw slug helper",
)
route_test = '''    #[test]
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
            tenant_route_scope("/v2/catalog/publish"),
            TenantRouteScope::TenantBound
        );
    }

'''
replace_once(route_test, "", "move route-policy test")
replace_once(
    '''    #[test]
    fn strict_header_mode_rejects_missing_header() {''',
    '''    #[test]
    fn explicit_slug_resolution_has_typed_handshake_source() {
        let resolution = resolve_explicit_slug("demo").expect("explicit slug resolution");
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("demo".to_string())
        );
        assert_eq!(
            resolution.source,
            TenantResolutionSource::SelfResolvingHandshake
        );
        assert_eq!(resolution.asserted_slug, None);
    }

    #[test]
    fn strict_header_mode_rejects_missing_header() {''',
    "add explicit slug source test",
)

path.write_text(source)
