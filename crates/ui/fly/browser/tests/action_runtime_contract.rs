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
        "switch_tab",
        "toggle_accordion",
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
    assert_contains("setVisible(target, willShow)");
    assert_contains("element.hidden = true");
    assert_contains("element.hidden = false");
}

#[test]
fn action_runtime_handles_tab_switching_and_aria_selected() {
    assert_contains("switchTab");
    assert_contains("tab.setAttribute(\"aria-selected\", isCurrent ? \"true\" : \"false\")");
    assert_contains("tab.classList.toggle(\"is-active\", isCurrent)");
    assert_contains("setVisible(panel, isTarget)");
    assert_contains("panel.setAttribute(\"aria-hidden\", isTarget ? \"false\" : \"true\")");
}

#[test]
fn action_runtime_handles_accordion_toggling_and_exclusive_mode() {
    assert_contains("toggleAccordion");
    assert_contains("action.exclusive === true");
    assert_contains("element.setAttribute(\"aria-expanded\", willExpand ? \"true\" : \"false\")");
    assert_contains("otherTrigger.setAttribute(\"aria-expanded\", \"false\")");
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
    assert_contains("data-fly-component-id");
    assert_contains("data-fly-style-id");
    assert_contains("data-fly-target-id");
    assert_contains("candidate.getAttribute(attribute) === id");
    assert_contains("candidate.closest(PAGE_ROOT)");
}

#[test]
fn action_groups_and_swipes_respect_landing_boundaries() {
    assert_contains("function belongsToPage(element, root)");
    assert_contains("root.contains(element) && pageRoot(element) === root");
    assert_contains("closestInPage(element, SLIDER_ROOT, pageRoot(element))");
    assert_contains("!belongsToPage(otherTrigger, root)");
    assert_contains("belongsToPage(tab, root) && tabOwner(tab, root) === tablist");
    assert_contains("hasForeignTablist(scope, tablist, panels, root)");
    assert_contains("!parentPanel.contains(tablist)");
    assert_contains("documentRef.addEventListener(\"touchcancel\", cancelTouch");
    assert_contains("!belongsToPage(touchSlider, touchPageRoot)");
}
