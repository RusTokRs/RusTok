use rustok_benchmarks::index_storage::{
    PartitionMutationConfig, capture_partition_mutation_evidence,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionMutationConfig::from_env()?;
    let capture = capture_partition_mutation_evidence(&config).await?;
    println!(
        "index partition mutation evidence complete: evidence_id={} runs={} samples_per_run={} output={}",
        capture.evidence_id,
        capture.runs,
        capture.samples_per_run,
        capture.output_path.display(),
    );
    Ok(())
}
