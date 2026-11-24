mod support;
use fly_browser::BrowserIntentKind;
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

#[test]
fn pending_operation_fence_does_not_block_read_only_intents() {
    let source = include_str!("../assets/fly-browser.js");
    let gate = source
        .find("this.pendingOperationUnresolved &&")
        .expect("pending operation fence");
    let condition = &source[gate..gate + 120];
    assert!(
        condition.contains("!READ_ONLY_INTENTS.has(request.intent)"),
        "only mutating intents should be blocked during receipt recovery"
    );
}

#[test]
fn strict_draft_operation_identity_survives_reload_and_is_reconciled_without_auto_retry() {
    let source = include_str!("../assets/fly-browser.js");
    assert!(source.contains("PENDING_OPERATION_PREFIX"));
    assert!(source.contains("writePendingOperation(this.pageId, request)"));
    assert!(source.contains("readPendingOperation(this.pageId)"));
    assert!(source.contains("this.resolvePendingOperation()"));
    assert!(source.contains("this.retryPendingOperation()"));
    assert!(source.contains("intent: \"resolve_operation\""));
    assert!(source.contains("preserveOriginalRequest: true"));
    assert!(source.contains("request.operation_id = createOperationId()"));
    assert!(source.contains("operation_status === \"committed\""));
    assert!(source.contains("operation_status === \"unknown\""));
    assert!(source.contains("this.pendingOperationUnresolved"));
    assert!(source.contains("The server has not confirmed this operation yet"));
    assert!(
        !source.contains("queueMicrotask(() => void this.retryPendingOperation())"),
        "ambiguous outcomes must never trigger an automatic mutation retry"
    );
}

#[test]
fn every_non_mutating_rust_intent_is_read_only_in_the_browser_receipt_policy() {
    let source = include_str!("../assets/fly-browser.js");
    let start = source
        .find("const READ_ONLY_INTENTS = new Set([")
        .expect("read-only intent set");
    let end = start
        + source[start..]
            .find("]);")
            .expect("read-only intent set end");
    let read_only = &source[start..end];
    for kind in BrowserIntentKind::ALL {
        if !kind.is_mutating() {
            let intent = format!("\"{}\"", kind.as_str());
            assert!(
                read_only.contains(&intent),
                "non-mutating intent `{}` must not consume durable operation receipts",
                kind.as_str()
            );
        }
    }
}
