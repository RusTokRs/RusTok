use rustok_benchmarks::index_storage::{BenchmarkConfig, run, write_provenance_bound_report};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = BenchmarkConfig::from_env()?;
    let report = run(&config).await?;
    write_provenance_bound_report(&config.output_path, &report, &config.run_provenance)?;

    println!(
        "index storage benchmark complete: scale={:?}, product_rows={}, output={}",
        config.dataset.scale,
        config.dataset.product_rows(),
        config.output_path.display()
    );
    for prototype in &report.prototypes {
        println!(
            "  {:?}: load={}ms size={} bytes workloads={}",
            prototype.prototype,
            prototype.load_ms,
            prototype.schema_bytes,
            prototype.workloads.len()
        );
    }

    Ok(())
}
