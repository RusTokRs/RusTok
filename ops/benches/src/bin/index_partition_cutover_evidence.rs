use rustok_benchmarks::index_storage::{
    PartitionCutoverConfig, capture_partition_cutover_evidence,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionCutoverConfig::from_env()?;
    let capture = capture_partition_cutover_evidence(&config).await?;
    println!(
        "index partition cutover evidence complete: evidence_id={} schema={} runs={} output={}",
        capture.evidence_id,
        capture.schema,
        capture.runs,
        capture.output_path.display(),
    );
    Ok(())
}
