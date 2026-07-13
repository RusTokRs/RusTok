/// Check whether a requested OAuth scope is admitted by an allowed scope list.
///
/// Wildcards are namespace-aware: `ai:*` admits `ai:providers:read`, while
/// `ai:providers:*` admits only scopes below the `ai:providers` namespace. A
/// textual prefix such as `ai2` never matches `ai`.
pub fn scope_matches(allowed: &[String], requested: &str) -> bool {
    let requested = requested.trim();
    if requested.is_empty() {
        return false;
    }

    allowed.iter().any(|allowed_scope| {
        let allowed_scope = allowed_scope.trim();
        if allowed_scope == "*:*" || allowed_scope == requested {
            return true;
        }

        let Some(namespace) = allowed_scope.strip_suffix(":*") else {
            return false;
        };
        !namespace.is_empty()
            && requested
                .strip_prefix(namespace)
                .is_some_and(|remainder| remainder.starts_with(':'))
    })
}

#[cfg(test)]
mod tests {
    use super::scope_matches;

    #[test]
    fn nested_namespace_wildcard_matches_only_its_descendants() {
        let allowed = vec!["ai:providers:*".to_string()];

        assert!(scope_matches(&allowed, "ai:providers:read"));
        assert!(scope_matches(&allowed, "ai:providers:write"));
        assert!(!scope_matches(&allowed, "ai:tasks:read"));
        assert!(!scope_matches(&allowed, "ai2:providers:read"));
    }

    #[test]
    fn top_level_namespace_wildcard_supports_nested_scopes() {
        let allowed = vec!["ai:*".to_string(), "catalog:*".to_string()];

        assert!(scope_matches(&allowed, "ai:providers:read"));
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(!scope_matches(&allowed, "catalogue:read"));
    }

    #[test]
    fn empty_or_malformed_values_do_not_match() {
        assert!(!scope_matches(&["ai:*".to_string()], ""));
        assert!(!scope_matches(&[":*".to_string()], ":read"));
    }
}
