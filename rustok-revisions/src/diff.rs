//! Diff computation for revisions.

use serde_json::{json, Value};

use crate::{Revision, RevisionDiff, RevisionError};

/// Compute the difference between two revisions.
pub fn compute_diff(from: &Revision, to: &Revision) -> Result<RevisionDiff, RevisionError> {
    let from_content = &from.content;
    let to_content = &to.content;

    let mut added = json!({});
    let mut removed = json!({});
    let mut changed = json!({});

    // Find added and changed fields
    if let (Some(from_obj), Some(to_obj)) = (from_content.as_object(), to_content.as_object()) {
        // Find added and changed fields
        for (key, to_value) in to_obj {
            match from_obj.get(key) {
                Some(from_value) => {
                    // Field exists in both, check if changed
                    if from_value != to_value {
                        changed.as_object_mut().unwrap().insert(
                            key.clone(),
                            json!({
                                "from": from_value,
                                "to": to_value
                            }),
                        );
                    }
                }
                None => {
                    // Field was added
                    added.as_object_mut().unwrap().insert(key.clone(), to_value.clone());
                }
            }
        }

        // Find removed fields
        for (key, from_value) in from_obj {
            if !to_obj.contains_key(key) {
                removed.as_object_mut().unwrap().insert(key.clone(), from_value.clone());
            }
        }
    }

    Ok(RevisionDiff {
        from_revision_id: from.id,
        to_revision_id: to.id,
        added,
        removed,
        changed,
    })
}

/// Apply a diff to content.
pub fn apply_diff(content: &Value, diff: &RevisionDiff) -> Result<Value, RevisionError> {
    let mut result = content.clone();

    if let Some(obj) = result.as_object_mut() {
        // Apply removals
        if let Some(removed_obj) = diff.removed.as_object() {
            for key in removed_obj.keys() {
                obj.remove(key);
            }
        }

        // Apply additions
        if let Some(added_obj) = diff.added.as_object() {
            for (key, value) in added_obj {
                obj.insert(key.clone(), value.clone());
            }
        }

        // Apply changes
        if let Some(changed_obj) = diff.changed.as_object() {
            for (key, change) in changed_obj {
                if let Some(to_value) = change.get("to") {
                    obj.insert(key.clone(), to_value.clone());
                }
            }
        }
    }

    Ok(result)
}
