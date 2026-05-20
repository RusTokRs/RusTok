#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("контрактные тесты покрывают все публичные use-case"),
        "implementation plan must include contract test checklist item"
    );
}
