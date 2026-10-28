use rustok_benchmarks::index_storage::{
    QueryEquivalenceAdmissionConfig, admit_query_equivalence_bundle,
};

fn main() -> anyhow::Result<()> {
    let config = QueryEquivalenceAdmissionConfig::from_env()?;
    let admission = admit_query_equivalence_bundle(&config)?;
    println!(
        "index query equivalence admission complete: commit={} run_key={} output={}",
        admission.commit,
        admission.run_key,
        admission.output_path.display(),
    );
    Ok(())
}
