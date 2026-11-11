use crate::manifest_io::load_manifest_from;
use anyhow::{ensure, Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};
use toml::Value;

/// Checks source packaging only. Runtime capabilities are verified separately.
pub(crate) fn lint(workspace: &Path, slug: &str, required: bool) -> Result<()> {
    let manifest = load_manifest_from(&workspace.join("modules.toml"))?;
    let owner = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown native path module '{slug}' in modules.toml"))?;
    let owner_path = owner.path.as_deref().map(Path::new);
    let owner_path = owner_path.filter(|path| {
        owner.source == "path"
            && !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    });
    let owner_path =
        owner_path.with_context(|| format!("Module '{slug}' must have a local owner path"))?;
    let transport_path = owner_path.join("transport");
    let transport = workspace.join(&transport_path);
    match fs::symlink_metadata(&transport) {
        Ok(metadata) => ensure!(
            metadata.is_dir(),
            "{} must be a directory",
            transport.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ensure!(
                !required,
                "Module '{slug}' is missing its canonical FBA package at {}",
                transport_path.display()
            );
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Cannot inspect {}", transport.display()))
        }
    }
    ensure!(
        transport
            .canonicalize()?
            .starts_with(workspace.canonicalize()?),
        "FBA package path escapes the workspace"
    );

    let owner_manifest = read_manifest(&workspace.join(owner_path).join("Cargo.toml"))?;
    let transport_manifest = read_manifest(&transport.join("Cargo.toml"))?;
    check_manifests(&owner.crate_name, &owner_manifest, &transport_manifest)?;

    let workspace_manifest = read_manifest(&workspace.join("Cargo.toml"))?;
    ensure!(
        workspace_manifest
            .get("workspace")
            .and_then(|table| table.get("members"))
            .is_some_and(|members| {
                strings(members).any(|member| Path::new(member) == transport_path.as_path())
            }),
        "Register {} explicitly in Cargo workspace.members",
        transport_path.display()
    );
    let transport_package = format!("{}-transport", owner.crate_name);
    ensure!(
        !manifest
            .modules
            .values()
            .any(|module| module.crate_name == transport_package)
            && !transport.join("rustok-module.toml").exists(),
        "The FBA package belongs to '{slug}' and must not have a separate module identity"
    );
    for path in ["build.rs", "src/lib.rs", "src/bin/service.rs"] {
        ensure!(
            transport.join(path).is_file(),
            "Missing FBA source {}",
            transport.join(path).display()
        );
    }
    ensure!(
        has_proto(&transport.join("proto"))?,
        "{} must contain an owner RPC schema",
        transport.join("proto").display()
    );
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Value> {
    let source =
        fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
    toml::from_str(&source).with_context(|| format!("Invalid TOML in {}", path.display()))
}

fn check_manifests(owner_name: &str, owner: &Value, transport: &Value) -> Result<()> {
    let transport_name = format!("{owner_name}-transport");
    ensure!(
        owner
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(Value::as_str)
            == Some(owner_name),
        "Owner Cargo package must be '{owner_name}'"
    );
    let package = transport
        .get("package")
        .context("FBA Cargo.toml is missing [package]")?;
    ensure!(
        package.get("name").and_then(Value::as_str) == Some(transport_name.as_str()),
        "FBA package must be named '{transport_name}'"
    );
    ensure!(
        package.get("autobins").and_then(Value::as_bool) == Some(false),
        "FBA package must set package.autobins = false"
    );
    ensure!(
        depends_on(transport, "dependencies", owner_name),
        "FBA package must depend on its owner '{owner_name}'"
    );
    for dependencies in ["dependencies", "build-dependencies", "dev-dependencies"] {
        let target_dependency =
            owner
                .get("target")
                .and_then(Value::as_table)
                .is_some_and(|targets| {
                    targets
                        .values()
                        .any(|target| depends_on(target, dependencies, &transport_name))
                });
        ensure!(
            !depends_on(owner, dependencies, &transport_name) && !target_dependency,
            "Owner '{owner_name}' must not depend on its FBA transport"
        );
    }

    let bins = transport
        .get("bin")
        .and_then(Value::as_array)
        .context("Declare the standalone service in [[bin]]")?;
    ensure!(
        bins.len() == 1,
        "FBA transport must declare exactly one standalone service binary"
    );
    let service = &bins[0];
    ensure!(
        service.get("path").and_then(Value::as_str) == Some("src/bin/service.rs")
            && service.get("name").and_then(Value::as_str).is_some_and(|name| !name.is_empty())
            && service.get("required-features").is_some_and(|value| strings(value).eq(["service"])),
        "FBA binary must have an explicit name, path = \"src/bin/service.rs\" and required-features = [\"service\"]"
    );
    let features = transport
        .get("features")
        .context("FBA package must declare [features]")?;
    ensure!(
        features.get("service").is_some_and(Value::is_array),
        "FBA package must declare the 'service' feature"
    );
    let mut pending = vec!["default"];
    let mut visited = HashSet::new();
    while let Some(feature) = pending.pop() {
        ensure!(
            feature != "service",
            "The FBA service feature must be opt-in"
        );
        if visited.insert(feature) {
            if let Some(next) = features.get(feature) {
                pending.extend(strings(next));
            }
        }
    }
    Ok(())
}

fn depends_on(manifest: &Value, section: &str, package: &str) -> bool {
    manifest
        .get(section)
        .and_then(Value::as_table)
        .is_some_and(|dependencies| {
            dependencies.iter().any(|(key, value)| {
                key == package || value.get("package").and_then(Value::as_str) == Some(package)
            })
        })
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

    const OWNER: &str = "[package]\nname = \"rustok-sample\"\n";
    const TRANSPORT: &str = r#"
[package]
name = "rustok-sample-transport"
autobins = false

[dependencies]
rustok-sample = { path = ".." }

[features]
service = []

[[bin]]
name = "rustok-sample-service"
path = "src/bin/service.rs"
required-features = ["service"]
"#;

    fn check(owner: &str, transport: &str) -> Result<()> {
        check_manifests(
            "rustok-sample",
            &toml::from_str(owner)?,
            &toml::from_str(transport)?,
        )
    }

    #[test]
    fn canonical_fba_package_uses_an_opt_in_binary() {
        check(OWNER, TRANSPORT).unwrap();
        let wrong_target = TRANSPORT.replace(
            "required-features = [\"service\"]",
            "required-features = []",
        );
        assert!(check(OWNER, &wrong_target)
            .unwrap_err()
            .to_string()
            .contains("required-features"));
        let default_service = TRANSPORT.replace(
            "[features]",
            "[features]\ndefault = [\"host\"]\nhost = [\"service\"]",
        );
        assert!(check(OWNER, &default_service)
            .unwrap_err()
            .to_string()
            .contains("opt-in"));
    }

    #[test]
    fn owner_cannot_depend_on_its_transport_through_an_alias() {
        let owner = format!("{OWNER}\n[target.'cfg(unix)'.dependencies]\nwire = {{ package = \"rustok-sample-transport\", path = \"transport\" }}\n");
        assert!(check(&owner, TRANSPORT)
            .unwrap_err()
            .to_string()
            .contains("must not depend"));
    }
}
