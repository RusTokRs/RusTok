use super::types::*;
use crate::services::build_service::ModuleSpec as BuildModuleSpec;
use std::path::{Path, PathBuf};

pub fn admin_frontend_build_plan(
    manifest: &ModulesManifest,
    cargo_profile: &str,
) -> Option<FrontendBuildPlan> {
    let admin_stack = manifest.build.admin.stack.trim().to_ascii_lowercase();
    let requires_leptos_admin = manifest.build.server.embed_admin || admin_stack == "leptos";

    requires_leptos_admin.then(|| {
        let mut command_parts = vec!["trunk".to_string(), "build".to_string()];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        }

        FrontendBuildPlan {
            surface: "admin".to_string(),
            tool: FrontendBuildTool::Trunk,
            package: "rustok-admin".to_string(),
            workspace_path: "apps/admin".to_string(),
            profile: cargo_profile.to_string(),
            target: None,
            artifact_path: "apps/admin/dist".to_string(),
            artifact_kind: FrontendArtifactKind::Directory,
            command: command_parts.join(" "),
        }
    })
}

pub fn storefront_frontend_build_plan(
    manifest: &ModulesManifest,
    cargo_profile: &str,
    cargo_target: Option<&str>,
) -> Option<FrontendBuildPlan> {
    let has_leptos_storefront = manifest.build.server.embed_storefront
        || manifest
            .build
            .storefront
            .iter()
            .any(|storefront| storefront.stack.trim().eq_ignore_ascii_case("leptos"));

    has_leptos_storefront.then(|| {
        let mut command_parts = vec![
            "cargo".to_string(),
            "build".to_string(),
            "-p".to_string(),
            "rustok-storefront".to_string(),
        ];
        if cargo_profile == "release" {
            command_parts.push("--release".to_string());
        } else {
            command_parts.push("--profile".to_string());
            command_parts.push(cargo_profile.to_string());
        }
        if let Some(target) = cargo_target {
            command_parts.push("--target".to_string());
            command_parts.push(target.to_string());
        }

        let mut artifact_path = String::from("target/");
        if let Some(target) = cargo_target {
            artifact_path.push_str(target);
            artifact_path.push('/');
        }
        artifact_path.push_str(binary_output_dir_name(cargo_profile));
        artifact_path.push('/');
        artifact_path.push_str(&binary_file_name("rustok-storefront", cargo_target));

        FrontendBuildPlan {
            surface: "storefront".to_string(),
            tool: FrontendBuildTool::Cargo,
            package: "rustok-storefront".to_string(),
            workspace_path: ".".to_string(),
            profile: cargo_profile.to_string(),
            target: cargo_target.map(ToString::to_string),
            artifact_path,
            artifact_kind: FrontendArtifactKind::File,
            command: command_parts.join(" "),
        }
    })
}

pub fn binary_output_dir_name(profile: &str) -> &str {
    if profile == "release" {
        "release"
    } else {
        profile
    }
}

pub fn binary_file_name(package: &str, cargo_target: Option<&str>) -> String {
    let exe_suffix = executable_suffix(cargo_target);
    if exe_suffix.is_empty() {
        package.to_string()
    } else {
        format!("{package}.{exe_suffix}")
    }
}

pub fn executable_suffix(cargo_target: Option<&str>) -> &'static str {
    match cargo_target {
        Some(target) if target.contains("windows") => "exe",
        Some(_) => "",
        None => std::env::consts::EXE_EXTENSION,
    }
}


