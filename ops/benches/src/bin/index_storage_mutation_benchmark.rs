use std::{env, path::PathBuf};

use rustok_benchmarks::index_storage::{BenchmarkConfig, run_mutations, write_mutation_report};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = BenchmarkConfig::from_env()?;
    let output = env::var("INDEX_BENCH_MUTATION_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/index-storage-benchmark/mutation-report.json"));
    let report = run_mutations(&config).await?;
    write_mutation_report(&output, &report)?;

    println!(
        "index mutation benchmark complete: scale={:?}, output={}",
        config.dataset.scale,
        output.display()
    );
    for prototype in &report.prototypes {
        println!(
            "  {:?}: mutation_workloads={}",
            prototype.prototype,
            prototype.workloads.len()
        );
    }

    Ok(())
}
