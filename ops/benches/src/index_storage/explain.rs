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
    let (shared_hit_blocks, shared_read_blocks) = required_direct_metric_pair(
        plan_node,
        "Shared Hit Blocks",
        "Shared Read Blocks",
        "shared buffer",
    )?;
    Ok(ReadExplainMetrics {
        planning_time_ms: required_non_negative_f64(root, "Planning Time")?,
        execution_time_ms: required_non_negative_f64(root, "Execution Time")?,
        shared_hit_blocks,
        shared_read_blocks,
        temporary_read_blocks: optional_non_negative_u64(plan_node, "Temp Read Blocks")?,
        temporary_written_blocks: optional_non_negative_u64(plan_node, "Temp Written Blocks")?,
    })
}

pub(crate) fn parse_mutation_explain_metrics(plan: &Value) -> Result<MutationExplainMetrics> {
    let (root, plan_node) = root_and_plan_node(plan)?;
    let (shared_hit_blocks, shared_read_blocks) = required_maximum_metric_pair(
        plan_node,
        "Shared Hit Blocks",
        "Shared Read Blocks",
        "shared buffer",
    )?;
    let (maximum_node_wal_records, maximum_node_wal_fpi, maximum_node_wal_bytes) =
        required_maximum_metric_triple(
            plan_node,
            "WAL Records",
            "WAL FPI",
            "WAL Bytes",
            "WAL",
        )?;
    Ok(MutationExplainMetrics {
        planning_time_ms: required_non_negative_f64(root, "Planning Time")?,
        execution_time_ms: required_non_negative_f64(root, "Execution Time")?,
        shared_hit_blocks,
        shared_read_blocks,
        temporary_read_blocks: maximum_metric(plan_node, "Temp Read Blocks")?,
        temporary_written_blocks: maximum_metric(plan_node, "Temp Written Blocks")?,
        maximum_node_wal_records,
        maximum_node_wal_fpi,
        maximum_node_wal_bytes,
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
    ensure!(
        value.is_finite() && value >= 0.0,
        "EXPLAIN {key} must be non-negative"
    );
    Ok(value)
}

fn required_direct_metric_pair(
    value: &Value,
    first_key: &str,
    second_key: &str,
    family: &str,
) -> Result<(u64, u64)> {
    let first = optional_non_negative_u64(value, first_key)?;
    let second = optional_non_negative_u64(value, second_key)?;
    ensure!(
        first.is_some() || second.is_some(),
        "EXPLAIN Plan is missing the {family} metric family"
    );
    Ok((first.unwrap_or(0), second.unwrap_or(0)))
}

fn required_maximum_metric_pair(
    value: &Value,
    first_key: &str,
    second_key: &str,
    family: &str,
) -> Result<(u64, u64)> {
    let first = maximum_metric(value, first_key)?;
    let second = maximum_metric(value, second_key)?;
    ensure!(
        first.is_some() || second.is_some(),
        "EXPLAIN plan tree is missing the {family} metric family"
    );
    Ok((first.unwrap_or(0), second.unwrap_or(0)))
}

fn required_maximum_metric_triple(
    value: &Value,
    first_key: &str,
    second_key: &str,
    third_key: &str,
    family: &str,
) -> Result<(u64, u64, u64)> {
    let first = maximum_metric(value, first_key)?;
    let second = maximum_metric(value, second_key)?;
    let third = maximum_metric(value, third_key)?;
    ensure!(
        first.is_some() || second.is_some() || third.is_some(),
        "EXPLAIN plan tree is missing the {family} metric family"
    );
    Ok((
        first.unwrap_or(0),
        second.unwrap_or(0),
        third.unwrap_or(0),
    ))
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
                "WAL Records": 1,
                "WAL Bytes": 10,
                "Plans": [
                    {"WAL Records": 2, "WAL FPI": 1, "WAL Bytes": 100},
                    {"Plans": [{"WAL Records": 1, "WAL Bytes": 75}]}
                ]
            }
        }]);
        let metrics = parse_mutation_explain_metrics(&plan).unwrap();
        assert_eq!(metrics.shared_read_blocks, 0);
        assert_eq!(metrics.maximum_node_wal_records, 2);
        assert_eq!(metrics.maximum_node_wal_fpi, 1);
        assert_eq!(metrics.maximum_node_wal_bytes, 100);
    }

    #[test]
    fn omitted_members_of_present_metric_family_become_zero() {
        let plan = serde_json::json!([{
            "Planning Time": 1.0,
            "Execution Time": 2.0,
            "Plan": {
                "Shared Hit Blocks": 3,
                "WAL Records": 4
            }
        }]);
        let metrics = parse_mutation_explain_metrics(&plan).unwrap();
        assert_eq!(metrics.shared_read_blocks, 0);
        assert_eq!(metrics.maximum_node_wal_fpi, 0);
        assert_eq!(metrics.maximum_node_wal_bytes, 0);
    }

    #[test]
    fn required_metric_family_fails_closed_when_absent() {
        let plan = serde_json::json!([{
            "Planning Time": 1.0,
            "Execution Time": 2.0,
            "Plan": {"Shared Hit Blocks": 0}
        }]);
        let error = parse_mutation_explain_metrics(&plan).unwrap_err();
        assert!(error.to_string().contains("WAL metric family"));
    }

    #[test]
    fn required_timing_fails_closed_when_missing() {
        let plan = serde_json::json!([{
            "Planning Time": 1.0,
            "Plan": {"Shared Hit Blocks": 0}
        }]);
        let error = parse_read_explain_metrics(&plan).unwrap_err();
        assert!(error.to_string().contains("Execution Time"));
    }
}
