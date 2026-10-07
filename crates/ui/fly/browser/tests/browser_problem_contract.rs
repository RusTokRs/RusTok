use fly_browser::FLY_BROWSER_ADAPTER_JS;

/// The bundle with all whitespace removed, so the contract survives formatter reflows.
fn compact_bundle() -> String {
    FLY_BROWSER_ADAPTER_JS
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

#[test]
fn public_bundle_reports_typed_accessible_browser_problems() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-problem"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("flyBrowserProblem"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("NETWORK_ERROR"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("normalizedProblem"));

    let compact = compact_bundle();
    // Problems are announced through an assertive alert region...
    assert!(
        compact.contains(r#"ensureStatus(adapter,PROBLEM_STATUS_SELECTOR,"problem","alert","assertive""#),
        "problem status must be an assertive alert region"
    );
    // ...and the shared status factory applies the requested role and live-region politeness.
    assert!(compact.contains(r#"status.setAttribute("role",role);"#));
    assert!(compact.contains(r#"status.setAttribute("aria-live",live);"#));
}
