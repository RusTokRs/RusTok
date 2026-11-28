mod support;
use support::contains;

const SOURCE: &str = include_str!("../assets/fly-browser.js");

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

#[test]
fn stopped_requests_report_abort_before_suppressing_late_rejections() {
    let post_intent = SOURCE
        .find("async postIntent(input, requestOptions = {})")
        .expect("postIntent implementation");
    let body = &SOURCE[post_intent..];
    let catch = body.find("    } catch (error) {").expect("postIntent catch");
    let body = &body[catch..];
    let abort = body
        .find("if (controller.signal.aborted) {")
        .expect("abort classification");
    let stopped = body
        .find("if (stopped) return null;")
        .expect("stopped adapter fence");
    assert!(abort < stopped);
    assert!(body.contains("const stopped = this.lifecycleState === ADAPTER_LIFECYCLE.STOPPED;"));
}
