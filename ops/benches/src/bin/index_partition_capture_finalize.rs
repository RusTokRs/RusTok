use rustok_benchmarks::index_storage::{
    PartitionCaptureFinalizeConfig, finalize_partition_capture,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PartitionCaptureFinalizeConfig::from_env()?;
    let capture = finalize_partition_capture(&config).await?;
    println!(
        "index partition capture descriptor complete: evidence_id={} output={}",
        capture.evidence_id,
        capture.output_path.display(),
    );
    Ok(())
}
