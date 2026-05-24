use sha2::Digest;

pub fn hash_manifest<T: serde::Serialize>(manifest: &T) -> Result<String, serde_json::Error> {
    let snapshot = serde_json::to_value(manifest)?;
    Ok(hash_manifest_snapshot(&snapshot))
}

pub fn hash_manifest_snapshot(snapshot: &serde_json::Value) -> String {
    let canonical_snapshot = canonicalize_json_value(snapshot);
    let serialized = serde_json::to_string(&canonical_snapshot).unwrap_or_default();
    let mut hasher = sha2::Sha256::new();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}

fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let canonical = entries
                .into_iter()
                .map(|(key, nested)| (key.clone(), canonicalize_json_value(nested)))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(canonicalize_json_value)
                .collect::<Vec<serde_json::Value>>(),
        ),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::hash_manifest_snapshot;

    #[test]
    fn manifest_snapshot_hash_is_sha256_hex() {
        let hash = hash_manifest_snapshot(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}}
        }));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn manifest_snapshot_hash_changes_when_snapshot_changes() {
        let left = hash_manifest_snapshot(&serde_json::json!({"a": 1}));
        let right = hash_manifest_snapshot(&serde_json::json!({"a": 2}));
        assert_ne!(left, right);
    }

    #[test]
    fn manifest_snapshot_hash_is_stable_for_different_object_key_order() {
        let left = hash_manifest_snapshot(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}, "pricing": {"enabled": false}},
            "profile": "default",
            "settings": {"b": 1, "a": 2}
        }));
        let right = hash_manifest_snapshot(&serde_json::json!({
            "settings": {"a": 2, "b": 1},
            "profile": "default",
            "modules": {"pricing": {"enabled": false}, "catalog": {"enabled": true}}
        }));
        assert_eq!(left, right);
    }
}


#[cfg(test)]
mod serialize_tests {
    use super::hash_manifest;

    #[test]
    fn hash_manifest_matches_snapshot_hash_for_serializable_input() {
        let manifest = serde_json::json!({"b": 2, "a": 1});
        let from_manifest = hash_manifest(&manifest).expect("serialize manifest");
        assert_eq!(
            from_manifest,
            super::hash_manifest_snapshot(&manifest),
            "typed manifest hashing must use the same canonical snapshot contract"
        );
    }
}
