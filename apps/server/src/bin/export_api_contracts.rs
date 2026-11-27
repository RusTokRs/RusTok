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
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(output_dir) = parse_output_dir_from_args(env::args_os().skip(1))? else {
        println!("Usage: export_api_contracts [--output-dir PATH]");
        return Ok(());
    };

    let settings = RustokSettings::default();
    if settings.runtime.is_registry_only() {
        return Err(
            "API compatibility export requires the full runtime host profile, not registry-only"
                .into(),
        );
    }

    let openapi = rustok_server::controllers::swagger::build_openapi_document(&settings);
    let openapi_json = serde_json::to_string_pretty(&openapi)?;

    let graphql_schema = Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .finish();
    let graphql_sdl = graphql_schema.sdl();

    fs::create_dir_all(&output_dir)?;
    write_contract(&output_dir.join("openapi.json"), &openapi_json)?;
    write_contract(&output_dir.join("schema.graphql"), &graphql_sdl)?;

    println!("exported API contracts to {}", output_dir.display());
    Ok(())
}

fn parse_output_dir_from_args<I, S>(args: I) -> Result<Option<PathBuf>, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut iter = args.into_iter();
    let mut output_dir = PathBuf::from("target/api-contracts");
    let mut output_dir_set = false;

    while let Some(argument) = iter.next() {
        let argument = argument.as_ref();
        if argument == OsStr::new("--output-dir") {
            if output_dir_set {
                return Err("duplicate --output-dir argument provided".into());
            }

            let value = iter.next().ok_or("--output-dir requires a path argument")?;
            let value = value.as_ref();
            output_dir = match value.to_str() {
                Some(value) => {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        return Err(
                            "--output-dir path must not be empty or whitespace".into(),
                        );
                    }
                    PathBuf::from(trimmed)
                }
                None if value == OsStr::new("") => {
                    return Err("--output-dir path must not be empty or whitespace".into());
                }
                None => PathBuf::from(value),
            };
            output_dir_set = true;
        } else if argument == OsStr::new("--help") || argument == OsStr::new("-h") {
            return Ok(None);
        } else {
            return Err(format!("unknown argument: {}", argument.to_string_lossy()).into());
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
    use std::ffi::OsString;

    #[test]
    fn defaults_to_target_api_contracts() {
        let result = parse_output_dir_from_args(Vec::<OsString>::new()).unwrap();
        assert_eq!(result, Some(PathBuf::from("target/api-contracts")));
    }

    #[test]
    fn parses_custom_output_dir() {
        let args = vec![
            OsString::from("--output-dir"),
            OsString::from("custom/contracts"),
        ];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, Some(PathBuf::from("custom/contracts")));
    }

    #[test]
    fn trims_whitespace_in_output_dir() {
        let args = vec![
            OsString::from("--output-dir"),
            OsString::from("  custom/contracts  "),
        ];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, Some(PathBuf::from("custom/contracts")));
    }

    #[test]
    fn returns_none_on_help_flag() {
        let args = vec![OsString::from("--help")];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, None);

        let args = vec![OsString::from("-h")];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn errors_on_missing_output_dir_argument() {
        let args = vec![OsString::from("--output-dir")];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(
            err.to_string()
                .contains("--output-dir requires a path argument")
        );
    }

    #[test]
    fn errors_on_empty_output_dir_argument() {
        let args = vec![OsString::from("--output-dir"), OsString::from("")];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(
            err.to_string()
                .contains("--output-dir path must not be empty or whitespace")
        );

        let args_whitespace = vec![
            OsString::from("--output-dir"),
            OsString::from("   "),
        ];
        let err_whitespace = parse_output_dir_from_args(args_whitespace).unwrap_err();
        assert!(
            err_whitespace
                .to_string()
                .contains("--output-dir path must not be empty or whitespace")
        );
    }

    #[test]
    fn errors_on_duplicate_output_dir_argument() {
        let args = vec![
            OsString::from("--output-dir"),
            OsString::from("dir1"),
            OsString::from("--output-dir"),
            OsString::from("dir2"),
        ];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(
            err.to_string()
                .contains("duplicate --output-dir argument provided")
        );
    }

    #[test]
    fn errors_on_unknown_argument() {
        let args = vec![OsString::from("--invalid-flag")];
        let err = parse_output_dir_from_args(args).unwrap_err();
        assert!(err.to_string().contains("unknown argument: --invalid-flag"));
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_unicode_output_directory() {
        use std::os::unix::ffi::OsStringExt;

        let path = OsString::from_vec(vec![
            b'c', b'o', b'n', b't', b'r', b'a', b'c', b't', 0xff,
        ]);
        let args = vec![OsString::from("--output-dir"), path.clone()];
        let result = parse_output_dir_from_args(args).unwrap();
        assert_eq!(result, Some(PathBuf::from(path)));
    }

    #[cfg(unix)]
    #[test]
    fn reports_non_unicode_unknown_arguments_without_panicking() {
        use std::os::unix::ffi::OsStringExt;

        let argument = OsString::from_vec(vec![0xff]);
        let err = parse_output_dir_from_args(vec![argument]).unwrap_err();
        assert!(err.to_string().contains("unknown argument:"));
    }

    #[test]
    fn write_contract_normalizes_crlf_and_appends_single_newline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("contract.txt");
        let raw = "line1\r\nline2\r\n\r\n";
        write_contract(&file_path, raw).unwrap();

        let read_back = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_back, "line1\nline2\n");
    }
}
