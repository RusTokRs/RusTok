use anyhow::{Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use toml::Value as TomlValue;

mod docs_registry_contracts;
mod manifest_io;
mod manifest_validation;
mod module_boundary_contracts;
mod module_cli_args;
mod module_commands;
mod module_contracts;
mod module_mutation_commands;
mod module_operation_previews;
mod module_permission_contracts;
mod module_publish_commands;
mod module_publish_readiness;
mod module_registry_commands;
mod module_registry_previews;
mod module_runner_commands;
mod module_runtime_contracts;
mod module_semantics;
mod module_transport_contracts;
mod module_ui_metadata_contracts;
mod permission_policy;
mod preview_builders;
mod registry_artifacts;
mod registry_http;
mod registry_lifecycle;
mod registry_publish_lifecycle;
mod registry_reason_codes;
mod registry_request_lifecycle;
mod registry_runner_lifecycle;
mod registry_runner_types;
mod registry_status;
mod registry_transport;
mod registry_types;
mod root_commands;
mod runtime_event_contracts;
mod runtime_parsing_helpers;
mod server_contracts;
mod server_event_runtime_contracts;
mod server_feature_contracts;
mod source_scan_helpers;
mod ui_contracts;
mod ui_dependency_contracts;
mod ui_feature_inventory_contracts;
mod ui_host_contracts;
mod ui_inventory_contracts;
mod ui_wiring_contracts;
mod validation_plans;
mod validation_remote_runner;
mod validation_runner;
mod validation_stage_metadata;
mod xtask_types;

use crate::manifest_validation::to_pascal_case;
use crate::xtask_types::*;
use docs_registry_contracts::*;
use manifest_io::*;
use module_boundary_contracts::*;
use module_cli_args::*;
use module_commands::*;
use module_contracts::*;
use module_mutation_commands::*;
use module_operation_previews::*;
use module_permission_contracts::*;
use module_publish_commands::*;
use module_publish_readiness::*;
use module_registry_commands::*;
use module_registry_previews::*;
use module_runner_commands::*;
use module_runtime_contracts::*;
use module_semantics::*;
use module_transport_contracts::*;
use module_ui_metadata_contracts::*;
use permission_policy::*;
use preview_builders::*;
use registry_artifacts::*;
use registry_http::*;
use registry_lifecycle::*;
use registry_publish_lifecycle::*;
use registry_reason_codes::*;
use registry_request_lifecycle::*;
use registry_runner_lifecycle::*;
use registry_runner_types::*;
use registry_status::*;
use registry_transport::*;
use registry_types::*;
use root_commands::*;
use runtime_event_contracts::*;
use runtime_parsing_helpers::*;
use server_contracts::*;
use server_event_runtime_contracts::*;
use server_feature_contracts::*;
use source_scan_helpers::*;
use ui_contracts::*;
use ui_dependency_contracts::*;
use ui_feature_inventory_contracts::*;
use ui_host_contracts::*;
use ui_inventory_contracts::*;
use ui_wiring_contracts::*;
use validation_plans::*;
use validation_remote_runner::*;
use validation_runner::*;
use validation_stage_metadata::*;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "generate-registry" => generate_registry()?,
        "validate-manifest" => validate_manifest()?,
        "list-modules" => list_modules()?,
        "module" => module_command(&args[2..])?,
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Usage: cargo xtask <command>");
    println!();
    println!("Commands:");
    println!("  generate-registry   Generate ModuleRegistry from modules.toml");
    println!("  validate-manifest   Validate modules.toml and rustok-module.toml files");
    println!("  list-modules        List all configured modules");
    println!("  module validate     Validate module publish-readiness contracts");
    println!("  module test         Run or preview local module smoke checks");
    println!("  module stage-run    Execute a local follow-up validation stage and report it");
    println!("  module runner       Run a thin remote validation worker against runner/* API");
    println!("  module publish      Create/preview a publish request and stop at review-ready unless --auto-approve is set");
    println!("  module request-changes Request a fresh artifact revision for an approved publish request");
    println!("  module hold         Place a publish request on hold");
    println!("  module resume       Resume a held publish request");
    println!("  module stage        Record or requeue a follow-up validation stage");
    println!("  module owner-transfer Emit a dry-run owner transfer payload preview");
    println!("  module yank         Emit a dry-run yank payload preview");
}

#[cfg(test)]
mod tests;
