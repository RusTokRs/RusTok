mod support;
use support::contains;

#[test]
fn public_bundle_orders_intent_responses_and_invalidates_on_stop() {
    assert!(contains("this.intentRequestGeneration = 0"));
    assert!(contains(
        "this.latestIntentRequestGeneration = requestGeneration"
    ));
    assert!(contains(
        "requestGeneration === this.latestIntentRequestGeneration"
    ));
    assert!(contains("if (current && response.ok && isObject(result))"));
    // `reportIntentTimeout`'s detail carries the request generation and whether it is still the
    // current one. The previous literal (`requestGeneration, current`) only matched because the
    // two adjacent fields happened to sit next to each other; this asserts the comparison itself.
    assert!(contains(
        "current: record.requestGeneration === this.latestIntentRequestGeneration"
    ));
    assert!(contains("event.detail?.current === false"));
    assert!(contains(
        "this.latestIntentRequestGeneration = ++this.intentRequestGeneration"
    ));
}
