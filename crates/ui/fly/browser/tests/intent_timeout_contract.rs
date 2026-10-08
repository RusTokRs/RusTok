mod support;
use support::contains;

#[test]
fn public_bundle_bounds_hung_intent_requests() {
    assert!(contains("DEFAULT_INTENT_REQUEST_TIMEOUT_MS"));
    assert!(contains("intentRequestTimeoutMs"));
    assert!(contains("flyIntentRequestTimeoutMs"));
    assert!(contains("INTENT_REQUEST_TIMEOUT"));
    assert!(contains("fly:browser-intent-timeout"));
    assert!(contains("clearTimeout"));
    // The timeout path aborts the in-flight request and carries the classified reason into the
    // abort call. `controller.abort()` matched the older call with no argument.
    assert!(contains(
        "this.reportIntentTimeout(record); controller.abort("
    ));
}
