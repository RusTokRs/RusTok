use std::{ffi::OsString, path::PathBuf};

use rustok_static_distribution_worker::{
    StaticDistributionPublisherPaths, run_static_distribution_publisher,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let paths = StaticDistributionPublisherPaths {
        request: required_path(&mut arguments, "--request")?,
        workspace: required_path(&mut arguments, "--workspace")?,
        test_evidence: required_path(&mut arguments, "--test-evidence")?,
        config: required_path(&mut arguments, "--config")?,
        config_digest: required_utf8(&mut arguments, "--config-digest")?,
        receipt: required_path(&mut arguments, "--receipt")?,
    };
    if arguments.next().is_some() {
        return Err("unexpected static-distribution publisher argument".into());
    }
    run_static_distribution_publisher(paths).await?;
    Ok(())
}

fn required_path(
    arguments: &mut impl Iterator<Item = OsString>,
    expected_flag: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let value = required_value(arguments, expected_flag)?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{expected_flag} path must be absolute").into());
    }
    Ok(path)
}

fn required_utf8(
    arguments: &mut impl Iterator<Item = OsString>,
    expected_flag: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    required_value(arguments, expected_flag)?
        .into_string()
        .map_err(|_| format!("{expected_flag} must be UTF-8").into())
}

fn required_value(
    arguments: &mut impl Iterator<Item = OsString>,
    expected_flag: &str,
) -> Result<OsString, Box<dyn std::error::Error>> {
    let flag = arguments
        .next()
        .ok_or("missing static-distribution publisher argument")?;
    if flag != OsString::from(expected_flag) {
        return Err(format!("expected static-distribution publisher flag {expected_flag}").into());
    }
    arguments
        .next()
        .ok_or_else(|| "missing static-distribution publisher value".into())
}
