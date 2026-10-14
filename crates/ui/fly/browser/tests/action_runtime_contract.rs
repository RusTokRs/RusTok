//! Contract tests for the published-page action runtime asset (`fly-actions.js`).
//!
//! These assert the behaviour-bearing fragments of the shipped JavaScript the same way the
//! adapter contracts do: token sequences, not raw formatting-sensitive text.

use fly_browser::FLY_ACTIONS_JS;

fn squashed(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect()
}

fn assert_contains(fragment: &str) {
    assert!(
        squashed(FLY_ACTIONS_JS).contains(&squashed(fragment)),
        "fly-actions.js must contain `{fragment}`"
    );
}

#[test]
fn action_runtime_handles_every_custom_action_kind() {
    for kind in [
        "emit_event",
        "provider_action",
        "toggle_visibility",
        "scroll_to",
    ] {
        assert_contains(&format!("case \"{kind}\""));
    }
}

#[test]
fn action_runtime_dispatches_bubbling_fly_action_events() {
    assert_contains(
        "new CustomEvent(\"fly:action\", { bubbles: true, cancelable: true, detail: detail })",
    );
    assert_contains("element.dispatchEvent(");
}

#[test]
fn action_runtime_keeps_aria_state_in_sync_for_toggles() {
    assert_contains("element.setAttribute(\"aria-expanded\", willShow ? \"true\" : \"false\")");
    assert_contains("target.setAttribute(\"aria-hidden\", willShow ? \"false\" : \"true\")");
    assert_contains("target.hidden = !willShow");
}

#[test]
fn action_runtime_is_idempotent_and_stoppable() {
    assert_contains("if (documentRef[ADAPTER_KEY]) return documentRef[ADAPTER_KEY];");
    assert_contains("documentRef.removeEventListener(\"click\", onClick)");
    assert_contains("globalThis.FlyActions = api");
}

#[test]
fn runtime_implements_allow_listed_animations_without_author_css() {
    assert_contains("case \"animate\"");
    for token in [
        "animateTarget",
        "fade_in",
        "fade_out",
        "slide_up",
        "slide_down",
        "zoom_in",
        "pulse",
        "ANIMATIONS[action.animation]",
        "prefers-reduced-motion",
        "target.animate(keyframes",
        "duration_ms",
    ] {
        assert_contains(token);
    }
}

#[test]
fn runtime_finds_targets_in_published_and_instrumented_renders() {
    assert_contains("root.getElementById(targetId)");
    assert_contains("[data-fly-component-id=\"");
}
