use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        normalize_registry_governance_event_payloads(manager.get_connection(), true).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        normalize_registry_governance_event_payloads(manager.get_connection(), false).await
    }
}

async fn normalize_registry_governance_event_payloads(
    db: &SchemaManagerConnection<'_>,
    forward: bool,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, details FROM registry_governance_events".to_string(),
        ))
        .await?;

    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let details = row.try_get::<Value>("", "details")?;
        let normalized = if forward {
            normalize_details_forward(details)
        } else {
            normalize_details_backward(details)
        };
        execute_statement(
            db,
            "UPDATE registry_governance_events SET details = {v1} WHERE id = {v2}",
            vec![normalized.into(), id.into()],
        )
        .await?;
    }

    Ok(())
}

fn normalize_details_forward(details: Value) -> Value {
    let Some(mut object) = details.as_object().cloned() else {
        return details;
    };

    let stage_key = object
        .get("stage_key")
        .cloned()
        .or_else(|| object.get("stage").cloned())
        .or_else(|| object.get("gate").cloned());

    object.remove("stage");
    object.remove("gate");
    if let Some(stage_key) = stage_key {
        object.insert("stage_key".to_string(), stage_key);
    }

    let owner_transition = normalized_owner_transition(&object);
    object.remove("previous_owner");
    object.remove("new_owner");
    object.remove("bound_by");
    object.remove("previous_owner_actor");
    object.remove("new_owner_actor");
    object.remove("owner_actor");
    if let Some(owner_transition) = owner_transition {
        object.insert("owner_transition".to_string(), owner_transition);
    } else {
        object.remove("owner_transition");
    }

    Value::Object(object)
}

fn normalize_details_backward(details: Value) -> Value {
    let Some(mut object) = details.as_object().cloned() else {
        return details;
    };

    let stage = object.remove("stage_key");
    if let Some(stage) = stage {
        object.insert("stage".to_string(), stage);
    }

    if let Some(owner_transition) = object.remove("owner_transition") {
        if let Some(transition) = owner_transition.as_object() {
            if let Some(previous_owner) = transition.get("previous_owner") {
                if let Some(label) = principal_label_value(previous_owner) {
                    object.insert(
                        "previous_owner_actor".to_string(),
                        Value::String(label.to_string()),
                    );
                }
            }
            if let Some(new_owner) = transition.get("new_owner") {
                if let Some(label) = principal_label_value(new_owner) {
                    object.insert(
                        "new_owner_actor".to_string(),
                        Value::String(label.to_string()),
                    );
                }
            }
            if let Some(bound_by) = transition.get("bound_by") {
                if let Some(label) = principal_label_value(bound_by) {
                    object.insert("bound_by".to_string(), Value::String(label.to_string()));
                }
            }
        }
    }

    Value::Object(object)
}

fn normalized_owner_transition(object: &Map<String, Value>) -> Option<Value> {
    let transition = object
        .get("owner_transition")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let previous_owner = transition
        .get("previous_owner")
        .or_else(|| object.get("previous_owner"))
        .or_else(|| object.get("previous_owner_actor"))
        .and_then(normalize_principal_value);

    let new_owner = transition
        .get("new_owner")
        .or_else(|| object.get("new_owner"))
        .or_else(|| object.get("new_owner_actor"))
        .or_else(|| object.get("owner_actor"))
        .and_then(normalize_principal_value);

    let bound_by = transition
        .get("bound_by")
        .or_else(|| object.get("bound_by"))
        .and_then(normalize_principal_value);

    if previous_owner.is_none() && new_owner.is_none() && bound_by.is_none() {
        return None;
    }

    let mut normalized = Map::new();
    if let Some(previous_owner) = previous_owner {
        normalized.insert("previous_owner".to_string(), previous_owner);
    }
    if let Some(new_owner) = new_owner {
        normalized.insert("new_owner".to_string(), new_owner);
    }
    if let Some(bound_by) = bound_by {
        normalized.insert("bound_by".to_string(), bound_by);
    }
    Some(Value::Object(normalized))
}

fn normalize_principal_value(value: &Value) -> Option<Value> {
    if value.is_null() {
        return None;
    }
    if value.get("kind").is_some() && value.get("subject").is_some() {
        return Some(value.clone());
    }
    principal_label_value(value).map(principal_json_from_label)
}

fn principal_label_value(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("display_label").and_then(Value::as_str))
        .or_else(|| value.get("displayLabel").and_then(Value::as_str))
        .or_else(|| value.get("subject").and_then(Value::as_str))
        .or_else(|| value.get("legacy_label").and_then(Value::as_str))
        .or_else(|| value.get("legacyLabel").and_then(Value::as_str))
}

fn principal_json_from_label(label: &str) -> Value {
    let normalized = label.trim();
    if let Some(raw_user_id) = normalized.strip_prefix("user:") {
        if let Ok(user_id) = Uuid::parse_str(raw_user_id) {
            return serde_json::json!({
                "kind": "user",
                "user_id": user_id,
                "subject": normalized,
                "display_label": normalized,
            });
        }
    }
    if let Some(runner_id) = normalized.strip_prefix("remote-runner:") {
        return serde_json::json!({
            "kind": "runner",
            "subject": format!("remote-runner:{runner_id}"),
            "display_label": format!("remote-runner:{runner_id}"),
        });
    }

    serde_json::json!({
        "kind": "legacy",
        "subject": normalized,
        "display_label": normalized,
        "legacy_label": normalized,
    })
}

async fn execute_statement(
    db: &SchemaManagerConnection<'_>,
    template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let sql = placeholder_sql(backend, template, values.len());
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    Ok(())
}

fn placeholder_sql(backend: DbBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DbBackend::Sqlite => format!("?{}", index + 1),
            _ => format!("${}", index + 1),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::{normalize_details_backward, normalize_details_forward};
    use serde_json::json;

    #[test]
    fn forward_normalization_rewrites_legacy_owner_and_stage_keys() {
        let normalized = normalize_details_forward(json!({
            "stage": "compile_smoke",
            "previous_owner_actor": "user:00000000-0000-0000-0000-000000000001",
            "owner_actor": "registry:admin",
            "bound_by": "user:00000000-0000-0000-0000-000000000002",
            "detail": "Owner was rebound."
        }));

        assert_eq!(normalized["stage_key"], "compile_smoke");
        assert!(normalized.get("stage").is_none());
        assert!(normalized.get("owner_actor").is_none());
        assert_eq!(
            normalized["owner_transition"]["previous_owner"]["kind"],
            "user"
        );
        assert_eq!(
            normalized["owner_transition"]["new_owner"]["kind"],
            "legacy"
        );
        assert_eq!(normalized["owner_transition"]["bound_by"]["kind"], "user");
    }

    #[test]
    fn backward_normalization_restores_flat_legacy_shape() {
        let normalized = normalize_details_backward(json!({
            "stage_key": "targeted_tests",
            "owner_transition": {
                "previous_owner": {
                    "kind": "user",
                    "user_id": "00000000-0000-0000-0000-000000000001",
                    "subject": "user:00000000-0000-0000-0000-000000000001",
                    "display_label": "user:00000000-0000-0000-0000-000000000001"
                },
                "new_owner": {
                    "kind": "legacy",
                    "subject": "registry:admin",
                    "display_label": "registry:admin",
                    "legacy_label": "registry:admin"
                }
            }
        }));

        assert_eq!(normalized["stage"], "targeted_tests");
        assert_eq!(
            normalized["previous_owner_actor"],
            "user:00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(normalized["new_owner_actor"], "registry:admin");
        assert!(normalized.get("owner_transition").is_none());
    }
}
