/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_graphql::Schema;
use rustok_server::common::settings::RustokSettings;
use rustok_server::graphql::schema::{Mutation, Query, Subscription};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(output_dir) = parse_output_dir_from_args(env::args().skip(1))? else {
        println!("Usage: export_api_contracts [--output-dir PATH]");
        return Ok(());
    };
    fs::create_dir_all(&output_dir)?;

    let settings = RustokSettings::default();
    if settings.runtime.is_registry_only() {
        return Err(
            "API compatibility export requires the full runtime host profile, not registry-only"
                .into(),
        );
    }

    let openapi = rustok_server::controllers::swagger::build_openapi_document(&settings);
    let openapi_json = serde_json::to_string_pretty(&openapi)?;
    write_contract(&output_dir.join("openapi.json"), &openapi_json)?;

    let graphql_schema = Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .finish();
    write_contract(&output_dir.join("schema.graphql"), &graphql_schema.sdl())?;

    println!("exported API contracts to {}", output_dir.display());
    Ok(())
}

fn parse_output_dir_from_args<I, S>(args: I) -> Result<Option<PathBuf>, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    let mut output_dir = PathBuf::from("target/api-contracts");

    while let Some(argument) = iter.next() {
        match argument.as_ref() {
            "--output-dir" => {
                let value = iter
                    .next()
                    .ok_or("--output-dir requires a path argument")?;
                output_dir = PathBuf::from(value.as_ref());
            }
            "--help" | "-h" => {
                return Ok(None);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    Ok(Some(output_dir))
}

fn write_contract(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let normalized = format!("{}\n", content.trim_end().replace("\r\n", "\n"));
    fs::write(path, normalized)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_target_api_contracts() {
        let result = parse_output_dir_from_args(Vec::<&str>::new()).unwrap();
        assert_eq!(result, Some(PathBuf::from("target/api-contracts")));
    }

    #[test]
    fn parses_custom_output_dir() {
        let args = vec!["--output-dir", "custom/contracts"];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, Some(PathBuf::from("custom/contracts")));
    }

    #[test]
    fn returns_none_on_help_flag() {
        let args = vec!["--help"];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, None);

        let args = vec!["-h"];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn errors_on_missing_output_dir_argument() {
        let args = vec!["--output-dir"];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(err.to_string().contains("--output-dir requires a path argument"));
    }

    #[test]
    fn errors_on_unknown_argument() {
        let args = vec!["--invalid-flag"];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(err.to_string().contains("unknown argument: --invalid-flag"));
    }
}

