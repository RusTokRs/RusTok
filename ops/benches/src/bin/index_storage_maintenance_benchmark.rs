use std::{env, path::PathBuf};

use anyhow::{Context, Result, ensure};
use rustok_benchmarks::index_storage::{
    BenchmarkConfig, run_maintenance, write_maintenance_report,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = BenchmarkConfig::from_env()?;
    let cycles = env::var("INDEX_BENCH_CHURN_CYCLES")
        .unwrap_or_else(|_| "5".to_owned())
        .parse::<u32>()
        .context("INDEX_BENCH_CHURN_CYCLES must be an integer")?;
    ensure!(cycles > 0, "INDEX_BENCH_CHURN_CYCLES must be greater than zero");
    let output = env::var("INDEX_BENCH_MAINTENANCE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("target/index-storage-benchmark/maintenance-report.json")
        });

    let report = run_maintenance(&config, cycles).await?;
    write_maintenance_report(&output, &report)?;

    println!(
        "index maintenance benchmark complete: scale={:?}, cycles={}, output={}",
        config.dataset.scale,
        cycles,
        output.display()
    );
    for prototype in &report.prototypes {
        println!(
            "  {:?}: baseline={} bytes, churn={} bytes, vacuum={}ms, after={} bytes",
            prototype.prototype,
            prototype.baseline.schema_bytes,
            prototype.after_churn.schema_bytes,
            prototype.vacuum_duration_ms,
            prototype.after_vacuum.schema_bytes
        );
    }

    Ok(())
}
