#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    let current_anchor = "Contract tests cover public event-contract use cases.";
    let legacy_anchor =
        "РєРѕРЅС‚СЂР°РєС‚РЅС‹Рµ С‚РµСЃС‚С‹ РїРѕРєСЂС‹РІР°СЋС‚ РІСЃРµ РїСѓР±Р»РёС‡РЅС‹Рµ use-case";

    assert!(
        plan.contains(current_anchor) || plan.contains(legacy_anchor),
        "implementation plan must include contract test checklist item"
    );
}
