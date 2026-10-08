mod support;
use support::contains;

#[test]
fn public_bundle_reports_typed_accessible_browser_problems() {
    assert!(contains("fly:browser-problem"));
    assert!(contains("flyBrowserProblem"));
    assert!(contains("NETWORK_ERROR"));
    // The problem status node is created by `ensureProblemStatus`, which passes the ARIA role and
    // the live-region politeness as arguments to `ensureStatus`. Asserting the rendered attribute
    // spellings (`role", "alert`) coupled the contract to one call shape; these assert the same
    // guarantee at the two places that actually produce it.
    assert!(contains("status.setAttribute(\"role\", role)"));
    assert!(contains("status.setAttribute(\"aria-live\", live)"));
    assert!(contains(
        "PROBLEM_STATUS_SELECTOR, \"problem\", \"alert\", \"assertive\""
    ));
    assert!(contains("normalizedProblem"));
}
