mod support;
use support::contains;

#[test]
fn public_bundle_bounds_and_aborts_pending_intents() {
    assert!(contains("DEFAULT_MAX_PENDING_INTENT_REQUESTS"));
    assert!(contains("maxPendingIntentRequests"));
    assert!(contains("PENDING_INTENT_LIMIT"));
    assert!(contains("fly:browser-intent-rejected"));
    assert!(contains("new AbortController()"));
    // Stopping the adapter aborts every still-pending intent with a classified
    // `ADAPTER_STOP` reason; the older `controller.abort()` call took no argument.
    assert!(contains("INTENT_ABORT_KIND.ADAPTER_STOP"));
    assert!(contains("record.controller.abort(record.abort.error)"));
    assert!(contains("signal: controller.signal"));
}
