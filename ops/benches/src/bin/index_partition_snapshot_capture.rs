use rustok_benchmarks::index_storage::{PartitionSnapshotConfig, capture_partition_snapshot};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionSnapshotConfig::from_env()?;
    let capture = capture_partition_snapshot(&config).await?;
    println!(
        "index partition snapshot capture complete: evidence_id={} baseline_rows={} shadow_rows={} baseline={} shadow={}",
        capture.evidence_id,
        capture.baseline_rows,
        capture.shadow_rows,
        capture.baseline_path.display(),
        capture.shadow_path.display(),
    );
    Ok(())
}
