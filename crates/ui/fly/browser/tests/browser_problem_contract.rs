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

/// A save rejected after the intent was applied arrives inside a successful response
/// (`save_failure`), together with `reload: true`. The adapter has to surface it as a problem and
/// keep it across that reload, otherwise the canvas silently snaps back to the server state.
#[test]
fn public_bundle_surfaces_save_failures_across_the_reload() {
    assert!(contains("const SAVE_FAILED_CODE = \"SAVE_FAILED\""));
    assert!(contains(
        "const SAVE_FAILURE_PREFIX = \"fly:ssr-save-failure:\""
    ));
    // Settled by the response listener, which runs before the reload is requested...
    assert!(contains(
        "if (event.detail?.ok === true) { settleSaveFailure(this, event.detail.result, event.detail.request); }"
    ));
    assert!(contains(
        "writeSaveFailure(adapter.pageId, result.save_failure); reportSaveFailure(adapter, result.save_failure, request);"
    ));
    // ...kept while the response still describes an unsaved working copy: a host does not have
    // to repeat the save of an unchanged document, so such a response carries no `save_failure`...
    assert!(contains(
        "const standing = state.dirty === true ? readSaveFailure(adapter.pageId) : null;"
    ));
    assert!(contains(
        "if (!standing) { clearSaveFailure(adapter.pageId); clearBrowserProblem(adapter); return; }"
    ));
    // ...not announced again while it is still on screen...
    assert!(contains(
        "if (adapter.root.dataset.flySaveFailed !== \"true\") { reportSaveFailure(adapter, standing, request); }"
    ));
    // ...and reported again by the adapter that starts after the reload.
    assert!(contains(
        "const pendingSaveFailure = readSaveFailure(this.pageId); if (pendingSaveFailure) reportSaveFailure(this, pendingSaveFailure, null);"
    ));
    // The parked copy is bounded in age and size.
    assert!(contains("age <= SAVE_FAILURE_MAX_AGE_MS"));
    assert!(contains(".slice(0, SAVE_FAILURE_MAX_ERROR_LENGTH,)"));
}
