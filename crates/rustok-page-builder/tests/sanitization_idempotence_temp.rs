use rustok_page_builder::sanitize_static_landing_project;
use serde_json::json;

#[test]
fn sanitizer_is_idempotent_for_rollback_recovery_fixture_shape() {
    let source = json!({
        "pages": [{
            "id": "home-en",
            "flyPageMeta": {
                "title": "Rollback activated A EN",
                "description": "Rollback-activated artifact-loss recovery",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Rollback activated A EN"
                }]
            }
        }]
    });

    let first = sanitize_static_landing_project(&source).expect("first sanitization");
    let second = sanitize_static_landing_project(first.project_data()).expect("second sanitization");

    eprintln!("first_hash={}", first.sanitized_hash());
    eprintln!("second_hash={}", second.sanitized_hash());
    if first.project_data() != second.project_data() {
        eprintln!(
            "first_project={}\nsecond_project={}",
            serde_json::to_string_pretty(first.project_data()).unwrap(),
            serde_json::to_string_pretty(second.project_data()).unwrap()
        );
    }

    assert_eq!(first.project_data(), second.project_data());
    assert_eq!(first.policy_format, second.policy_format);
    assert_eq!(first.policy_hash, second.policy_hash);
    assert_eq!(first.sanitized_hash(), second.sanitized_hash());
}
