#[test]
fn reference_plan_distinguishes_acceptance_gates_from_execution_evidence() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("### Remaining acceptance gates")
            && plan.contains("### Verification record")
            && plan.contains("applicable-reference-evidence"),
        "the reference plan must track applicable gates and execution separately"
    );
    assert!(
        !plan.contains("Contract tests cover every public use case."),
        "a documentation marker must not certify blanket behavioral coverage"
    );
}
