use rustok_benchmarks::index_storage::{QueryEquivalenceCaptureConfig, capture_query_equivalence};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = QueryEquivalenceCaptureConfig::from_env()?;
    let capture = capture_query_equivalence(&config).await?;
    println!(
        "index query equivalence capture complete: commit={} run_key={} output={}",
        capture.commit,
        capture.run_key,
        capture.output_root.display(),
    );
    Ok(())
}
