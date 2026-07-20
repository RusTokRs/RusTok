#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("Lock the public mock and fixture contract"),
        "implementation plan must include contract test checklist item"
    );
}
