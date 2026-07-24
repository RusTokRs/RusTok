use anyhow::{Context, Result, ensure};
use serde_json::{Map, Value};

#[derive(Debug)]
pub(crate) struct ReadExplainMetrics {
    pub planning_time_ms: f64,
    pub execution_time_ms: f64,
    pub shared_hit_blocks: u64,
    pub shared_read_blocks: u64,
    pub temporary_read_blocks: Option<u64>,
    pub temporary_written_blocks: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct MutationExplainMetrics {
    pub planning_time_ms: f64,
    pub execution_time_ms: f64,
    pub shared_hit_blocks: u64,
    pub shared_read_blocks: u64,
    pub temporary_read_blocks: Option<u64>,
    pub temporary_written_blocks: Option<u64>,
    pub maximum_node_wal_records: u64,
    pub maximum_node_wal_fpi: u64,
    pub maximum_node_wal_bytes: u64,
}

pub(crate) fn parse_read_explain_metrics(plan: &Value) -> Result<ReadExplainMetrics> {
    let (root, plan_node) = root_and_plan_node(plan)?;
    Ok(ReadExplainMetrics {
        planning_time_ms: required_non_negative_f64(root, "Planning Time")?,
        execution_time_ms: required_non_negative_f64(root, "Execution Time")?,
        shared_hit_blocks: required_non_negative_u64(plan_node, "Shared Hit Blocks")?,
        shared_read_blocks: required_non_negative_u64(plan_node, "Shared Read Blocks")?,
        temporary_read_blocks: optional_non_negative_u64(plan_node, "Temp Read Blocks")?,
        temporary_written_blocks: optional_non_negative_u64(plan_node, "Temp Written Blocks")?,
    })
}

pub(crate) fn parse_mutation_explain_metrics(plan: &Value) -> Result<MutationExplainMetrics> {
    let (root, plan_node) = root_and_plan_node(plan)?;
    Ok(MutationExplainMetrics {
        planning_time_ms: required_non_negative_f64(root, "Planning Time")?,
        execution_time_ms: required_non_negative_f64(root, "Execution Time")?,
        shared_hit_blocks: required_maximum_metric(plan_node, "Shared Hit Blocks")?,
        shared_read_blocks: required_maximum_metric(plan_node, "Shared Read Blocks")?,
        temporary_read_blocks: maximum_metric(plan_node, "Temp Read Blocks")?,
        temporary_written_blocks: maximum_metric(plan_node, "Temp Written Blocks")?,
        maximum_node_wal_records: required_maximum_metric(plan_node, "WAL Records")?,
        maximum_node_wal_fpi: required_maximum_metric(plan_node, "WAL FPI")?,
        maximum_node_wal_bytes: required_maximum_metric(plan_node, "WAL Bytes")?,
    })
}

fn root_and_plan_node(plan: &Value) -> Result<(&Map<String, Value>, &Value)> {
    let entries = plan
        .as_array()
        .context("EXPLAIN result must be a JSON array")?;
    ensure!(
        entries.len() == 1,
        "EXPLAIN result must contain exactly one root entry"
    );
    let root = entries[0]
        .as_object()
        .context("EXPLAIN root entry must be a JSON object")?;
    let plan_node = root
        .get("Plan")
        .filter(|value| value.is_object())
        .context("EXPLAIN root entry must contain a Plan object")?;
    Ok((root, plan_node))
}

fn required_non_negative_f64(root: &Map<String, Value>, key: &str) -> Result<f64> {
    let value = root
        .get(key)
        .and_then(Value::as_f64)
        .with_context(|| format!("EXPLAIN root is missing numeric {key}"))?;
    ensure!(value.is_finite() && value >= 0.0, "EXPLAIN {key} must be non-negative");
    Ok(value)
}

fn required_non_negative_u64(value: &Value, key: &str) -> Result<u64> {
    optional_non_negative_u64(value, key)?
        .with_context(|| format!("EXPLAIN Plan is missing non-negative integer {key}"))
}

fn optional_non_negative_u64(value: &Value, key: &str) -> Result<Option<u64>> {
    let Some(metric) = value.get(key) else {
        return Ok(None);
    };
    if metric.is_null() {
        return Ok(None);
    }
    metric
        .as_u64()
        .map(Some)
        .with_context(|| format!("EXPLAIN metric {key} must be a non-negative integer"))
}

fn required_maximum_metric(value: &Value, key: &str) -> Result<u64> {
    maximum_metric(value, key)?
        .with_context(|| format!("EXPLAIN plan tree is missing non-negative integer {key}"))
}

fn maximum_metric(value: &Value, key: &str) -> Result<Option<u64>> {
    let own = optional_non_negative_u64(value, key)?;
    let nested = match value {
        Value::Array(values) => {
            let mut maximum = None;
            for value in values {
                maximum = maximum.into_iter().chain(maximum_metric(value, key)?).max();
            }
            maximum
        }
        Value::Object(values) => {
            let mut maximum = None;
            for value in values.values() {
                maximum = maximum.into_iter().chain(maximum_metric(value, key)?).max();
            }
            maximum
        }
        _ => None,
    };
    Ok(own.into_iter().chain(nested).max())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_metrics_find_nested_wal_value() {
        let plan = serde_json::json!([{
            "Planning Time": 1.0,
            "Execution Time": 2.0,
            "Plan": {
                "Shared Hit Blocks": 10,
                "Shared Read Blocks": 0,
                "WAL Records": 1,
                "WAL FPI": 0,
                "WAL Bytes": 10,
                "Plans": [
                    {"WAL Records": 2, "WAL FPI": 1, "WAL Bytes": 100},
                    {"Plans": [{"WAL Records": 1, "WAL FPI": 0, "WAL Bytes": 75}]}
                ]
            }
        }]);
        let metrics = parse_mutation_explain_metrics(&plan).unwrap();
        assert_eq!(metrics.maximum_node_wal_records, 2);
        assert_eq!(metrics.maximum_node_wal_fpi, 1);
        assert_eq!(metrics.maximum_node_wal_bytes, 100);
    }

    #[test]
    fn required_metrics_fail_closed_when_missing() {
        let plan = serde_json::json!([{
            "Planning Time": 1.0,
            "Plan": {"Shared Hit Blocks": 0, "Shared Read Blocks": 0}
        }]);
        let error = parse_read_explain_metrics(&plan).unwrap_err();
        assert!(error.to_string().contains("Execution Time"));
    }
}
