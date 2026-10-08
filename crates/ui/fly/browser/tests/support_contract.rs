mod support;
use support::contains;

#[test]
fn folding_helper_ignores_formatting_but_not_content() {
    assert_eq!(
        support::fold("ensureStatus(\n    adapter,\n)"),
        support::fold("ensureStatus(adapter,)")
    );
    assert_ne!(support::fold("a b"), support::fold("a-b"));
    assert_eq!(support::fold("role\", \"alert"), "role\",\"alert");
}

#[test]
fn shipped_adapter_is_reachable_and_matchable() {
    assert!(contains("export function bootstrapFlyBrowsers"));
    assert!(!contains(
        "a fragment that is definitely absent from the asset"
    ));
}
