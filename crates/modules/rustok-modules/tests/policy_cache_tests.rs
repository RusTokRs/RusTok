use std::sync::Arc;
use uuid::Uuid;

use rustok_modules::{ModuleEffectivePolicy, ModuleEffectivePolicyCache};

fn sample_policy(digest_char: char, enabled: &[&str]) -> ModuleEffectivePolicy {
    let revision = format!("sha256:{}", digest_char.to_string().repeat(64));
    serde_json::from_value(serde_json::json!({
        "policy_revision": revision,
        "enabled_modules": enabled,
        "decisions": {}
    }))
    .expect("deserialize sample policy")
}

#[test]
fn test_cache_hit_on_matching_revision() {
    let cache = ModuleEffectivePolicyCache::new();
    let tenant_id = Uuid::new_v4();
    let policy = sample_policy('a', &["auth", "pages"]);
    let expected_revision = policy.policy_revision().to_string();

    let identity = cache.insert(tenant_id, policy.clone()).expect("insert");
    assert_eq!(identity.tenant_id(), tenant_id);
    assert_eq!(identity.policy_revision(), expected_revision.as_str());

    let hit = cache.get(tenant_id, &expected_revision);
    assert_eq!(hit, Some(policy.clone()));

    let (latest, latest_identity, _cached_at) =
        cache.get_with_metadata(tenant_id).expect("metadata");
    assert_eq!(latest, policy);
    assert_eq!(latest_identity, identity);
}

#[test]
fn test_cache_miss_fail_closed_on_mismatched_revision() {
    let cache = ModuleEffectivePolicyCache::new();
    let tenant_id = Uuid::new_v4();
    let policy = sample_policy('a', &["auth"]);
    cache.insert(tenant_id, policy).expect("insert");

    let wrong_revision = format!("sha256:{}", "b".repeat(64));
    let miss = cache.get(tenant_id, &wrong_revision);
    assert!(miss.is_none());
}

#[test]
fn test_cache_invalidation_methods() {
    let cache = ModuleEffectivePolicyCache::new();
    let tenant_id = Uuid::new_v4();
    let policy = sample_policy('a', &["auth"]);
    let revision = policy.policy_revision().to_string();

    cache.insert(tenant_id, policy).expect("insert");
    assert_eq!(cache.len(), 1);

    // Invalidate stale with different revision should not remove
    assert!(!cache.invalidate_if_stale(tenant_id, &format!("sha256:{}", "c".repeat(64))));
    assert_eq!(cache.len(), 1);

    // Invalidate stale with exact revision should remove
    assert!(cache.invalidate_if_stale(tenant_id, &revision));
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_cache_apply_transition_event() {
    let cache = ModuleEffectivePolicyCache::new();
    let tenant_id = Uuid::new_v4();
    let policy_a = sample_policy('a', &["auth"]);
    let rev_a = policy_a.policy_revision().to_string();
    let rev_b = format!("sha256:{}", "b".repeat(64));

    cache.insert(tenant_id, policy_a).expect("insert");
    assert_eq!(cache.len(), 1);

    // Transition from A to B purges entry
    let evicted = cache.apply_transition_event(tenant_id, Some(&rev_a), &rev_b);
    assert!(evicted);
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_cache_thread_safety_and_concurrent_access() {
    let cache = Arc::new(ModuleEffectivePolicyCache::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let cache_clone = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            let tenant_id = Uuid::from_u128(i as u128 + 1);
            let policy = sample_policy((b'0' + (i as u8 % 10)) as char, &["auth"]);
            let rev = policy.policy_revision().to_string();
            cache_clone.insert(tenant_id, policy).expect("insert");
            let retrieved = cache_clone.get(tenant_id, &rev);
            assert!(retrieved.is_some());
        }));
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    assert_eq!(cache.len(), 10);
    cache.clear();
    assert!(cache.is_empty());
}
