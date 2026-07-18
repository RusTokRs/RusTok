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
    let output_dir = parse_output_dir()?;
    fs::create_dir_all(&output_dir)?;

    let openapi = rustok_server::controllers::swagger::build_openapi_document(
        &RustokSettings::default(),
    );
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

fn parse_output_dir() -> Result<PathBuf, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut output_dir = PathBuf::from("target/api-contracts");

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output-dir" => {
                let value = args
                    .next()
                    .ok_or("--output-dir requires a path argument")?;
                output_dir = PathBuf::from(value);
            }
            "--help" | "-h" => {
                println!("Usage: export_api_contracts [--output-dir PATH]");
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    Ok(output_dir)
}

fn write_contract(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let normalized = format!("{}\n", content.trim_end().replace("\r\n", "\n"));
    fs::write(path, normalized)?;
    Ok(())
}
