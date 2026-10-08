mod support;
use support::contains;

#[test]
fn bundled_adapter_uses_request_scoped_abort_signals() {
    assert!(contains("async postIntent(input, requestOptions = {})"));
    // Request-scoped abort signals are funnelled through `normalizedTransportOptions`, which
    // validates the caller's signal before it reaches `forwardAbortSignal`; the previous literal
    // (`signal: requestOptions?.signal`) described an earlier inline shape of the same contract.
    assert!(contains("normalizedTransportOptions(requestOptions)"));
    assert!(contains(
        "signal: isAbortSignal(transport.signal) ? transport.signal : undefined"
    ));
    assert!(contains("signal: controller.signal"));
    assert!(contains("forwardAbortSignal"));
    assert!(!contains("globalThis.fetch ="));
}
