mod support;
use support::contains;

#[test]
fn bundled_adapter_owns_typed_abort_classification() {
    assert!(contains("IntentTransportOptions"));
    assert!(contains("IntentAbortMetadata"));
    assert!(contains("normalizedTransportOptions"));
    assert!(contains("signal?: AbortSignal"));
    assert!(contains("newAbortMetadata"));
    assert!(contains("fly:browser-intent-aborted"));
    assert!(contains("INTENT_REQUEST_ABORTED"));
    assert!(contains("adapter_stop"));
    assert!(contains("NETWORK_ERROR"));
    assert!(!contains("abort?: IntentAbortMetadata"));
    assert!(!contains("transport.abort"));
    assert!(!contains("pendingIntentRecordForGeneration"));
    assert!(!contains("reportIntentAborted"));
}
