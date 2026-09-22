use rustok_benchmarks::index_storage::{PartitionQueryConfig, capture_partition_query_evidence};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionQueryConfig::from_env()?;
    let capture = capture_partition_query_evidence(&config).await?;
    println!(
        "index partition query evidence complete: evidence_id={} runs={} samples_per_run={} output={}",
        capture.evidence_id,
        capture.runs,
        capture.samples_per_run,
        capture.output_path.display(),
    );
    Ok(())
}
