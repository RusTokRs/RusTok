use crate::manifest_io::load_manifest_from;
use anyhow::{Context, Result, ensure};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};
use toml::Value;

const SERVICE_BIN: &str = "src/bin/service.rs";
const GRPC_FEATURES: [&str; 2] = ["grpc-client", "grpc-server"];

/// Checks declared dependencies and source placement. Runtime profiles need separate evidence.
pub(crate) fn lint(workspace: &Path, slug: &str, required: bool) -> Result<()> {
    let manifest = load_manifest_from(&workspace.join("modules.toml"))?;
    let owner = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown native path module '{slug}' in modules.toml"))?;
    let owner_path = owner.path.as_deref().map(Path::new).filter(|path| {
        owner.source == "path"
            && !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    });
    let owner_path =
        owner_path.with_context(|| format!("Module '{slug}' must have a local owner path"))?;
    let owner_root = workspace.join(owner_path);
    let ports_path = owner_path.join("ports");
    let ports_root = workspace.join(&ports_path);
    match fs::symlink_metadata(&ports_root) {
        Ok(metadata) => ensure!(
            metadata.is_dir(),
            "{} must be a directory",
            ports_root.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ensure!(
                !required,
                "Module '{slug}' is missing its canonical FBA package at {}",
                ports_path.display()
            );
            if owner_root.join(SERVICE_BIN).exists() {
                let owner_manifest = read_manifest(&owner_root.join("Cargo.toml"))?;
                check_service(&owner_manifest, &owner_root, None)?;
            }
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Cannot inspect {}", ports_root.display()));
        }
    }

    let resolved_ports = ports_root.canonicalize()?;
    ensure!(
        resolved_ports.starts_with(workspace.canonicalize()?),
        "FBA package path escapes the workspace"
    );
    let owner_manifest = read_manifest(&owner_root.join("Cargo.toml"))?;
    let ports_manifest = read_manifest(&ports_root.join("Cargo.toml"))?;
    let workspace_manifest = read_manifest(&workspace.join("Cargo.toml"))?;
    let workspace_table = workspace_manifest
        .get("workspace")
        .context("Root Cargo.toml is missing [workspace]")?;
    ensure!(
        workspace_table.get("members").is_some_and(
            |members| strings(members).any(|member| Path::new(member) == ports_path.as_path())
        ),
        "Register {} explicitly in Cargo workspace.members",
        ports_path.display()
    );

    let owner_name = owner.crate_name.as_str();
    let ports_name = format!("{owner_name}-ports");
    ensure!(
        !manifest.modules.values().any(|module| {
            module.crate_name == ports_name
                || module
                    .path
                    .as_deref()
                    .is_some_and(|path| Path::new(path) == ports_path)
        }) && !ports_root.join("rustok-module.toml").exists(),
        "The FBA ports package belongs to '{slug}' and must not have a separate module identity"
    );
    ensure!(
        !owner_root.join("transport/Cargo.toml").exists(),
        "Move the former nested transport package into the canonical FBA layout"
    );
    ensure!(
        ports_root.join("src/lib.rs").is_file(),
        "Missing FBA source {}",
        ports_root.join("src/lib.rs").display()
    );
    ensure!(
        ports_manifest.get("bin").is_none()
            && !ports_root.join("src/bin").exists()
            && !ports_root.join("src/main.rs").exists(),
        "Standalone binaries belong to the owner, not its ports package"
    );

    let registered_owners: HashSet<&str> = manifest
        .modules
        .values()
        .map(|module| module.crate_name.as_str())
        .collect();
    let catalog = workspace_table.get("dependencies");
    check_manifests(
        owner_name,
        &owner_manifest,
        &ports_manifest,
        catalog,
        &registered_owners,
    )?;
    let (ports_key, definition, inherited) =
        owner_ports_dependency(&owner_manifest, catalog, &ports_name)?;
    let base = if inherited { workspace } else { &owner_root };
    let declared_path = definition
        .get("path")
        .and_then(Value::as_str)
        .context("The owner must depend on its local ports package by path")?;
    ensure!(
        base.join(declared_path).canonicalize()? == resolved_ports,
        "The owner dependency '{ports_key}' must refer to {}",
        ports_path.display()
    );

    let grpc = GRPC_FEATURES
        .iter()
        .any(|feature| has_feature(&ports_manifest, feature))
        || ports_root.join("proto").exists()
        || ports_root.join("build.rs").exists()
        || ports_root.join("src/grpc").exists();
    if grpc {
        ensure!(
            GRPC_FEATURES
                .iter()
                .all(|feature| has_feature(&ports_manifest, feature)),
            "FBA gRPC ports must declare both 'grpc-client' and 'grpc-server' features"
        );
        ensure!(
            ports_root.join("build.rs").is_file()
                && ports_root.join("src/grpc").is_dir()
                && has_proto(&ports_root.join("proto"))?,
            "FBA gRPC ports require build.rs, src/grpc/ and an owner schema under proto/"
        );
    }
    check_service(&owner_manifest, &owner_root, grpc.then_some(ports_key))?;
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Value> {
    let source =
        fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
    toml::from_str(&source).with_context(|| format!("Invalid TOML in {}", path.display()))
}

fn check_manifests(
    owner_name: &str,
    owner: &Value,
    ports: &Value,
    catalog: Option<&Value>,
    registered_owners: &HashSet<&str>,
) -> Result<()> {
    let ports_name = format!("{owner_name}-ports");
    for (manifest, name) in [(owner, owner_name), (ports, ports_name.as_str())] {
        ensure!(
            manifest
                .get("package")
                .and_then(|package| package.get("name"))
                .and_then(Value::as_str)
                == Some(name),
            "FBA Cargo package must be named '{name}'"
        );
    }

    for section in dependency_scopes(owner) {
        for kind in ["dependencies", "build-dependencies"] {
            for (key, dependency) in dependencies(section, kind) {
                let (package, _) = resolve_dependency(key, dependency, catalog)?;
                ensure!(
                    !registered_owners.contains(package) && package != "rustok-server",
                    "Owner '{owner_name}' declares a dependency on runtime '{package}'; bind its public ports instead"
                );
            }
        }
    }

    let default_ports_features = enabled_features(ports, "default");
    ensure!(
        GRPC_FEATURES
            .iter()
            .all(|feature| !default_ports_features.contains(feature)),
        "FBA gRPC features must be opt-in"
    );
    for section in dependency_scopes(ports) {
        for kind in ["dependencies", "build-dependencies", "dev-dependencies"] {
            for (key, dependency) in dependencies(section, kind) {
                let (package, definition) = resolve_dependency(key, dependency, catalog)?;
                ensure!(
                    package != owner_name
                        && !registered_owners.contains(package)
                        && !matches!(
                            package,
                            "rustok-server"
                                | "rustok-runtime"
                                | "sea-orm"
                                | "sea-orm-migration"
                                | "sqlx"
                                | "postgres"
                                | "tokio-postgres"
                                | "diesel"
                                | "rusqlite"
                        ),
                    "FBA ports must not depend on runtime or storage package '{package}'"
                );
                if package == "rustok-api" {
                    let runtime_features = ["runtime", "server"];
                    ensure!(
                        [definition, dependency].into_iter().all(|dep| dep
                            .get("features")
                            .into_iter()
                            .flat_map(strings)
                            .all(|feature| !runtime_features.contains(&feature)))
                            && ports
                                .get("features")
                                .and_then(Value::as_table)
                                .into_iter()
                                .flat_map(|features| features.values())
                                .flat_map(strings)
                                .all(|feature| {
                                    let Some((dep, enabled)) = feature.split_once('/') else {
                                        return true;
                                    };
                                    dep.trim_end_matches('?') != key
                                        || !runtime_features.contains(&enabled)
                                }),
                        "FBA ports must use rustok-api without its runtime or server features"
                    );
                }
                if kind != "dev-dependencies"
                    && matches!(
                        package,
                        "tonic"
                            | "tonic-prost"
                            | "prost"
                            | "prost-types"
                            | "protoc-bin-vendored"
                            | "tonic-prost-build"
                            | "tonic-build"
                            | "prost-build"
                    )
                {
                    ensure!(
                        dependency.get("optional").and_then(Value::as_bool) == Some(true)
                            && !default_ports_features.iter().any(|feature| {
                                let feature = feature.strip_prefix("dep:").unwrap_or(feature);
                                feature
                                    .split('/')
                                    .next()
                                    .unwrap_or(feature)
                                    .trim_end_matches('?')
                                    == key
                            }),
                        "FBA gRPC dependency '{key}' must be optional and disabled by default"
                    );
                }
            }
        }
    }
    Ok(())
}

fn check_service(owner: &Value, root: &Path, grpc_ports_key: Option<&str>) -> Result<()> {
    let mut service = owner
        .get("bin")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|bin| bin.get("path").and_then(Value::as_str) == Some(SERVICE_BIN));
    let bin = service.next();
    if bin.is_none() && !root.join(SERVICE_BIN).exists() && !has_feature(owner, "service") {
        return Ok(());
    }
    let bin =
        bin.context("Declare the standalone service at src/bin/service.rs in owner [[bin]]")?;
    ensure!(
        service.next().is_none()
            && root.join(SERVICE_BIN).is_file()
            && owner
                .get("package")
                .and_then(|package| package.get("autobins"))
                .and_then(Value::as_bool)
                == Some(false)
            && bin
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| !name.is_empty())
            && bin
                .get("required-features")
                .is_some_and(|value| strings(value).eq(["service"])),
        "The owner service requires an explicit [[bin]] name, path = \"src/bin/service.rs\", required-features = [\"service\"] and package.autobins = false"
    );
    ensure!(
        has_feature(owner, "service") && !enabled_features(owner, "default").contains("service"),
        "The owner must declare an opt-in 'service' feature"
    );
    if let Some(key) = grpc_ports_key {
        ensure!(
            enabled_features(owner, "service").contains(format!("{key}/grpc-server").as_str()),
            "The owner 'service' feature must enable '{key}/grpc-server'"
        );
    }
    Ok(())
}

fn owner_ports_dependency<'a>(
    owner: &'a Value,
    catalog: Option<&'a Value>,
    ports_name: &str,
) -> Result<(&'a str, &'a Value, bool)> {
    for (key, dependency) in dependencies(owner, "dependencies") {
        let (package, definition) = resolve_dependency(key, dependency, catalog)?;
        if package == ports_name {
            ensure!(
                dependency.get("optional").and_then(Value::as_bool) != Some(true),
                "The owner must use its ports package in the base build"
            );
            return Ok((
                key,
                definition,
                dependency.get("workspace").and_then(Value::as_bool) == Some(true),
            ));
        }
    }
    anyhow::bail!("Owner [dependencies] must include '{ports_name}'")
}

fn dependency_scopes(manifest: &Value) -> impl Iterator<Item = &Value> {
    std::iter::once(manifest).chain(
        manifest
            .get("target")
            .and_then(Value::as_table)
            .into_iter()
            .flat_map(|targets| targets.values()),
    )
}

fn dependencies<'a>(scope: &'a Value, kind: &str) -> impl Iterator<Item = (&'a str, &'a Value)> {
    scope
        .get(kind)
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .map(|(key, dependency)| (key.as_str(), dependency))
}

fn resolve_dependency<'a>(
    key: &'a str,
    dependency: &'a Value,
    catalog: Option<&'a Value>,
) -> Result<(&'a str, &'a Value)> {
    let definition = if dependency.get("workspace").and_then(Value::as_bool) == Some(true) {
        catalog
            .and_then(|table| table.get(key))
            .with_context(|| format!("Missing inherited Cargo dependency '{key}'"))?
    } else {
        dependency
    };
    Ok((
        definition
            .get("package")
            .and_then(Value::as_str)
            .unwrap_or(key),
        definition,
    ))
}

fn has_feature(manifest: &Value, feature: &str) -> bool {
    manifest
        .get("features")
        .and_then(|table| table.get(feature))
        .is_some_and(Value::is_array)
}

fn enabled_features<'a>(manifest: &'a Value, root: &'a str) -> HashSet<&'a str> {
    let features = manifest.get("features");
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(feature) = pending.pop() {
        if visited.insert(feature) {
            if let Some(next) = features.and_then(|table| table.get(feature)) {
                pending.extend(strings(next));
            }
        }
    }
    visited
}

fn strings(value: &Value) -> impl Iterator<Item = &str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn has_proto(dir: &Path) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "proto")
            || kind.is_dir() && has_proto(&entry.path())?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = r#"
[package]
name = "rustok-sample"
autobins = false

[dependencies]
owner-contract = { package = "rustok-sample-ports", path = "ports" }
"#;
    const PORTS: &str = "[package]\nname = \"rustok-sample-ports\"\n";
    const CATALOG: &str = r#"
private-owner = { package = "rustok-peer", path = "crates/modules/rustok-peer" }
"#;

    #[test]
    fn ports_dependency_is_local_and_base_builds_need_no_grpc() {
        let owner = toml::from_str(OWNER).unwrap();
        let ports = toml::from_str(PORTS).unwrap();
        check_manifests("rustok-sample", &owner, &ports, None, &HashSet::new()).unwrap();
        let (key, definition, inherited) =
            owner_ports_dependency(&owner, None, "rustok-sample-ports").unwrap();
        assert_eq!(key, "owner-contract");
        assert_eq!(
            definition.get("path").and_then(Value::as_str),
            Some("ports")
        );
        assert!(!inherited);
    }

    #[test]
    fn inherited_target_dependency_cannot_link_foreign_runtime() {
        let owners = HashSet::from(["rustok-sample", "rustok-peer"]);
        let catalog = toml::from_str(CATALOG).unwrap();
        let owner = toml::from_str(OWNER).unwrap();
        let ports = toml::from_str(&format!(
            "{PORTS}\n[target.'cfg(unix)'.build-dependencies]\nprivate-owner.workspace = true\n"
        ))
        .unwrap();
        assert!(
            check_manifests("rustok-sample", &owner, &ports, Some(&catalog), &owners)
                .unwrap_err()
                .to_string()
                .contains("rustok-peer")
        );
        let owner = toml::from_str(&format!(
            "{OWNER}\n[target.'cfg(unix)'.dependencies]\nprivate-owner.workspace = true\n"
        ))
        .unwrap();
        let ports = toml::from_str(PORTS).unwrap();
        assert!(
            check_manifests("rustok-sample", &owner, &ports, Some(&catalog), &owners)
                .unwrap_err()
                .to_string()
                .contains("bind its public ports")
        );
    }

    #[test]
    fn base_ports_profile_rejects_grpc_through_default_alias() {
        let owner = toml::from_str(OWNER).unwrap();
        let ports = toml::from_str(&format!(
            "{PORTS}\n[features]\ndefault = [\"combined\"]\ncombined = [\"grpc-client\"]\ngrpc-client = []\n"
        ))
        .unwrap();
        assert!(
            check_manifests("rustok-sample", &owner, &ports, None, &HashSet::new())
                .unwrap_err()
                .to_string()
                .contains("opt-in")
        );
    }
}
