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
fn strict_receipt_fence_blocks_view_state_intents_before_dom_dispatch() {
    assert!(contains(
        "requiresOperationReceipts && intent !== \"resolve_operation\""
    ));
    assert!(contains(
        "const authoritative = isAuthoritativeIntent(request.intent, this.requiresOperationReceipts)"
    ));
    let source = include_str!("../assets/fly-browser.js");
    let emit = source
        .find("emitIntent(intent, payload = {})")
        .expect("emit intent");
    let body = &source[emit..];
    let fence = body
        .find("this.pendingOperationUnresolved")
        .expect("receipt fence");
    let dispatch = body
        .find("new CustomEvent(\"fly:browser-intent\"")
        .expect("DOM dispatch");
    assert!(fence < dispatch);
}

#[test]
fn strict_draft_operation_identity_survives_reload_and_is_reconciled_without_auto_retry() {
    let source = include_str!("../assets/fly-browser.js");
    assert!(source.contains("PENDING_OPERATION_PREFIX"));
    assert!(source.contains("writePendingOperation(this.pageId, request, retryPending)"));
    assert!(source.contains("samePendingRequest(readPendingOperation(pageId)?.request, request)"));
    assert!(source.contains("isCommittedOperationReceipt(result, this.pendingOperation.request)"));
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
fn strict_mount_uses_server_rendered_draft_identity_not_stale_storage() {
    let source = include_str!("../assets/fly-browser.js");
    assert!(source.contains("this.requiresOperationReceipts = root.dataset.flyDraftContract ==="));
    assert!(source.contains("authoritative: root.dataset.flyDraftBinding ==="));
    assert!(source.contains("readDraftSession(this.pageId, {"));
    assert!(source.contains("token: root.dataset.flyRenderedDraftToken"));
    assert!(source.contains("generation: root.dataset.flyRenderedDraftGeneration"));
    let rendered = source
        .find("renderedBinding.required === true")
        .expect("strict binding");
    let storage = source
        .find("globalThis.sessionStorage?.getItem(draftStorageKey(pageId))")
        .expect("legacy storage fallback");
    assert!(
        rendered < storage,
        "strict mounts must not consult stale storage first"
    );
    assert!(source.contains(
        "this.root.dataset.flyRenderedDraftGeneration = String(this.draftSession.generation)"
    ));
}

#[test]
fn strict_requests_require_a_page_id_before_they_can_be_sent() {
    let source = include_str!("../assets/fly-browser.js");
    assert!(source.contains("!nonEmptyString(request.page_id)"));
    assert!(source.contains("!nonEmptyString(pendingRequest.page_id)"));
}

#[test]
fn committed_receipts_are_bound_to_the_original_page_and_draft() {
    let source = include_str!("../assets/fly-browser.js");
    let start = source
        .find("function isCommittedOperationReceipt")
        .expect("committed receipt validator");
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset)
        .expect("validator end");
    let validator = &source[start..end];
    assert!(validator.contains("nonEmptyString(request.page_id) !== null"));
    assert!(validator.contains("state.page_id === request.page_id"));
    assert!(validator.contains("result.draft_token === request.draft_token"));
    assert!(validator.contains("result.operation_id === request.operation_id"));
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
                "non-mutating intent `{}` must be classified for the non-strict transport",
                kind.as_str()
            );
        }
    }
}
