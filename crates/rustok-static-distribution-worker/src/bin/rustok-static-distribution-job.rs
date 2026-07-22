use std::{ffi::OsString, path::PathBuf};

use rustok_static_distribution_worker::{run_static_distribution_job, StaticDistributionJobPaths};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let paths = StaticDistributionJobPaths {
        job_request: required_path(&mut arguments, "--job-request")?,
        generated_manifest: required_path(&mut arguments, "--generated-manifest")?,
        cargo_dependencies: required_path(&mut arguments, "--cargo-dependencies")?,
        registry_source: required_path(&mut arguments, "--registry-source")?,
        job_config: required_path(&mut arguments, "--job-config")?,
        job_receipt: required_path(&mut arguments, "--receipt")?,
    };
    if arguments.next().is_some() {
        return Err("unexpected static-distribution launcher argument".into());
    }
    run_static_distribution_job(paths).await?;
    Ok(())
}

fn required_path(
    arguments: &mut impl Iterator<Item = OsString>,
    expected_flag: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let flag = arguments
        .next()
        .ok_or("missing static-distribution launcher argument")?;
    if flag != OsString::from(expected_flag) {
        return Err(format!("expected static-distribution launcher flag {expected_flag}").into());
    }
    let path = PathBuf::from(
        arguments
            .next()
            .ok_or("missing static-distribution launcher path")?,
    );
    if !path.is_absolute() {
        return Err(format!("{expected_flag} path must be absolute").into());
    }
    Ok(path)
}
