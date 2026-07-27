use rustok_benchmarks::index_storage::{
    PartitionMaintenanceConfig, capture_partition_maintenance_evidence,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionMaintenanceConfig::from_env()?;
    let capture = capture_partition_maintenance_evidence(&config).await?;
    println!(
        "index partition maintenance evidence complete: evidence_id={} schema={} runs={} output={}",
        capture.evidence_id,
        capture.schema,
        capture.runs,
        capture.output_path.display(),
    );
    Ok(())
}
