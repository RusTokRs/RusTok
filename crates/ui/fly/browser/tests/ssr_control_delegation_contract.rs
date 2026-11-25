mod support;
use support::contains;

#[test]
fn nested_component_selection_uses_one_nearest_canvas_target() {
    assert!(contains("this.root.addEventListener(\"click\", (event) => {"));
    assert!(contains("event.target.closest(\"[data-fly-canvas] [data-fly-component-id]\")"));
    assert!(contains("this.select(componentId, \"ssr-control\")"));
    assert!(contains("event.preventDefault()"));
    assert!(!contains("element.addEventListener(\"click\", () => this.select(componentId, \"ssr-control\")"));
}

#[test]
fn nested_component_drag_uses_one_nearest_component_target() {
    assert!(contains("this.root.addEventListener(\"dragstart\", (event) => {"));
    assert!(contains("event.target.closest(\"[data-fly-component-id]\")"));
    assert!(contains("this.root.addEventListener(\"dragend\", () => this.cancelDrag()"));
    assert!(!contains("element.addEventListener(\"dragstart\", (event) => { if (!componentId) return;"));
}


#[test]
fn browser_selection_is_forwarded_as_a_read_only_intent() {
    assert!(contains("void this.emitIntent(\"select\", { component_id: next });"));
}


#[test]
fn hydrated_workspace_actions_and_selection_have_one_transport_owner() {
    assert!(contains("actionElement.closest(\"[data-fly-wasm-hydrated='true']\")"));
    assert!(contains("componentElement.closest(\"[data-fly-wasm-hydrated='true']\")"));
    assert!(contains("source === \"ssr-control\" && this.root.querySelector(\"[data-fly-wasm-hydrated='true']\")"));
}


#[test]
fn selection_intents_do_not_reapply_the_selected_component_hint() {
    assert!(contains("intent !== \"select\""));
    assert!(contains("intent !== \"focus_requested\""));
}
