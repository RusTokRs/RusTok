//! Partial, deterministic Cargo resolution evidence. This is not a build read trace.

use super::*;

const ROOT_MANIFEST: &str = "crates/utils/rustok-migrations/Cargo.toml";
pub(super) const BINARIES: [&str; 2] = ["rustok-migrate", "export-migration-plan"];

#[derive(Deserialize)]
struct Metadata {
    version: u32,
    workspace_root: PathBuf,
    workspace_members: Vec<String>,
    packages: Vec<Package>,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
    source: Option<String>,
    manifest_path: PathBuf,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
    src_path: PathBuf,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    features: Vec<String>,
    deps: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DependencyKind {
    kind: Option<String>,
    target: Option<String>,
}

#[derive(Serialize)]
pub(super) struct BuildInputs {
    source_sha256: String,
    lockfile_sha256: String,
    configuration_inputs: Vec<SourceFile>,
    host_target: String,
    root_package: String,
    packages: BTreeMap<String, ResolvedPackage>,
    coverage: Coverage,
}

#[derive(Serialize)]
struct Coverage {
    resolution: &'static str,
    feature_precision: &'static str,
    unverified: [&'static str; 5],
}

#[derive(Serialize)]
struct ResolvedPackage {
    name: String,
    version: String,
    source: PackageSource,
    features: BTreeSet<String>,
    targets: BTreeSet<ResolvedTarget>,
    dependencies: BTreeSet<ResolvedDependency>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PackageSource {
    Snapshot { manifest: String },
    External { identity: String },
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct ResolvedTarget {
    name: String,
    kind: BTreeSet<String>,
    crate_types: BTreeSet<String>,
    source_path: String,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct ResolvedDependency {
    name: String,
    package: String,
    kinds: BTreeSet<DependencyKind>,
}

pub(super) fn native_target(rustc_version: &str) -> Result<String> {
    let hosts: Vec<_> = rustc_version
        .lines()
        .filter_map(|line| line.strip_prefix("host: "))
        .collect();
    ensure!(
        hosts.len() == 1,
        "rustc must report exactly one host target"
    );
    let host = hosts[0];
    ensure!(
        !host.is_empty()
            && host.len() <= 128
            && host.as_bytes()[0].is_ascii_alphanumeric()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
        "unsupported rustc host target"
    );
    Ok(host.to_owned())
}

pub(super) fn collect(
    source: &Path,
    snapshot: &SourceSnapshot,
    target: &Path,
    host: &str,
) -> Result<BuildInputs> {
    let mut metadata = build_command("cargo", source)?;
    metadata
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--filter-platform",
            host,
        ])
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_BUILD_JOBS", "2");
    let bytes = run_process(&mut metadata, 120, 64 * 1024 * 1024)
        .context("cannot collect migration build resolution")?;
    project(&bytes, source, snapshot, host)
}

fn source_path(root: &Path, path: &Path) -> Result<String> {
    let root = root.canonicalize().context("Cargo source root missing")?;
    let path = path.canonicalize().context("Cargo source input missing")?;
    let relative = path
        .strip_prefix(&root)
        .context("Cargo path escapes captured source")?;
    let value = relative
        .to_str()
        .context("Cargo path is not UTF-8")?
        .replace('\\', "/");
    safe_relative(&value)?;
    Ok(value)
}

fn captured<'a>(files: &'a BTreeMap<&str, &'a SourceFile>, path: &str) -> Result<&'a SourceFile> {
    files
        .get(path)
        .copied()
        .with_context(|| format!("Cargo input is absent from snapshot: {path}"))
}

fn project(
    bytes: &[u8],
    root: &Path,
    snapshot: &SourceSnapshot,
    host: &str,
) -> Result<BuildInputs> {
    let metadata: Metadata = serde_json::from_slice(bytes).context("invalid Cargo resolution")?;
    ensure!(metadata.version == 1, "unsupported Cargo metadata version");
    ensure!(
        metadata.workspace_root.canonicalize()? == root.canonicalize()?,
        "Cargo resolved another workspace"
    );
    let files: BTreeMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let lockfile_sha256 = captured(&files, "Cargo.lock")?.sha256.clone();
    captured(&files, "Cargo.toml")?;
    let mut packages = BTreeMap::new();
    for package in &metadata.packages {
        ensure!(
            packages.insert(package.id.as_str(), package).is_none(),
            "duplicate Cargo package"
        );
    }
    let mut nodes = BTreeMap::new();
    for node in &metadata.resolve.nodes {
        ensure!(
            nodes.insert(node.id.as_str(), node).is_none(),
            "duplicate Cargo resolve node"
        );
    }
    let roots: Vec<_> = metadata
        .packages
        .iter()
        .filter(|package| package.name == "rustok-migrations" && package.source.is_none())
        .collect();
    ensure!(
        roots.len() == 1,
        "canonical migration package missing or ambiguous"
    );
    ensure!(
        roots[0].manifest_path.canonicalize()? == root.join(ROOT_MANIFEST).canonicalize()?,
        "unexpected migration manifest"
    );
    let root_id = roots[0].id.as_str();
    ensure!(
        metadata.workspace_members.iter().any(|id| id == root_id),
        "migration package is not a workspace member"
    );

    let mut selected = BTreeSet::new();
    let mut pending = vec![root_id];
    while let Some(id) = pending.pop() {
        if !selected.insert(id) {
            continue;
        }
        ensure!(
            packages.contains_key(id),
            "Cargo dependency package missing"
        );
        let node = nodes
            .get(id)
            .context("Cargo dependency resolve node missing")?;
        for dependency in &node.deps {
            ensure!(
                !dependency.dep_kinds.is_empty(),
                "Cargo dependency has no kind"
            );
            for kind in &dependency.dep_kinds {
                ensure!(
                    matches!(kind.kind.as_deref(), None | Some("build" | "dev")),
                    "unsupported Cargo dependency kind"
                );
            }
            // This is a native normal/build closure, not a test build. Cargo metadata
            // can still unify features with other workspace/dev consumers.
            if dependency
                .dep_kinds
                .iter()
                .any(|kind| kind.kind.as_deref() != Some("dev"))
            {
                pending.push(dependency.pkg.as_str());
            }
        }
    }
    let mut identities = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for id in &selected {
        let package = packages[id];
        ensure!(
            !package.name.is_empty() && !package.version.is_empty(),
            "empty Cargo package identity"
        );
        let source = match &package.source {
            Some(identity) => {
                ensure!(!identity.is_empty(), "empty external Cargo source identity");
                PackageSource::External {
                    identity: identity.clone(),
                }
            }
            None => {
                let manifest = source_path(root, &package.manifest_path)?;
                captured(&files, &manifest)?;
                PackageSource::Snapshot { manifest }
            }
        };
        let identity = digest(&serde_json::to_vec(&(
            &package.name,
            &package.version,
            &source,
        ))?);
        ensure!(
            !identities.values().any(|existing| existing == &identity),
            "ambiguous normalized Cargo package identity"
        );
        identities.insert(*id, identity);
        sources.insert(*id, source);
    }
    let mut resolved = BTreeMap::new();
    for id in &selected {
        let package = packages[id];
        let node = nodes[id];
        let mut targets = BTreeSet::new();
        ensure!(!package.targets.is_empty(), "Cargo package has no targets");
        for target in package.targets.iter().filter(|target| {
            target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib"
                        | "rlib"
                        | "dylib"
                        | "cdylib"
                        | "staticlib"
                        | "proc-macro"
                        | "custom-build"
                )
            }) || (*id == root_id
                && target.kind.iter().any(|kind| kind == "bin")
                && BINARIES.contains(&target.name.as_str()))
        }) {
            let source_path = if package.source.is_none() {
                let path = source_path(root, &target.src_path)?;
                captured(&files, &path)?;
                path
            } else {
                source_path(
                    package
                        .manifest_path
                        .parent()
                        .context("external Cargo manifest parent missing")?,
                    &target.src_path,
                )?
            };
            ensure!(
                targets.insert(ResolvedTarget {
                    name: target.name.clone(),
                    kind: target.kind.iter().cloned().collect(),
                    crate_types: target.crate_types.iter().cloned().collect(),
                    source_path,
                }),
                "duplicate Cargo target"
            );
        }
        ensure!(
            !targets.is_empty(),
            "Cargo package has no selected build targets"
        );
        let mut dependencies = BTreeSet::new();
        for dependency in &node.deps {
            let kinds: BTreeSet<_> = dependency
                .dep_kinds
                .iter()
                .filter(|kind| kind.kind.as_deref() != Some("dev"))
                .cloned()
                .collect();
            if kinds.is_empty() {
                continue;
            }
            dependencies.insert(ResolvedDependency {
                name: dependency.name.clone(),
                package: identities
                    .get(dependency.pkg.as_str())
                    .context("Cargo dependency identity missing")?
                    .clone(),
                kinds,
            });
        }
        resolved.insert(
            identities[id].clone(),
            ResolvedPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                source: sources
                    .remove(id)
                    .context("Cargo source identity missing")?,
                features: node.features.iter().cloned().collect(),
                targets,
                dependencies,
            },
        );
    }
    Ok(BuildInputs {
        source_sha256: snapshot.sha256.clone(),
        lockfile_sha256,
        configuration_inputs: snapshot
            .files
            .iter()
            .filter(|file| {
                matches!(
                    file.path.as_str(),
                    "Cargo.toml"
                        | ".cargo/config"
                        | ".cargo/config.toml"
                        | "rust-toolchain"
                        | "rust-toolchain.toml"
                )
            })
            .cloned()
            .collect(),
        host_target: host.to_owned(),
        root_package: identities[root_id].clone(),
        packages: resolved,
        coverage: Coverage {
            resolution: "native normal/build dependency closure from filtered workspace metadata",
            feature_precision: "workspace resolution may include features from unbuilt workspace/dev consumers; not an exact compilation unit graph",
            unverified: [
                "external package bytes",
                "generated inputs",
                "build-script and proc-macro effects",
                "ambient native/toolchain inputs",
                "per-migration source attribution",
            ],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    struct Fixture {
        directory: PathBuf,
        root: PathBuf,
        snapshot: SourceSnapshot,
    }

    impl Fixture {
        fn new() -> Result<Self> {
            let directory = std::env::temp_dir().canonicalize()?.join(format!(
                "rustok-build-input-test-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
            ));
            let root = directory.join("source");
            fs::create_dir_all(&root)?;
            for (path, bytes) in [
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/utils/rustok-migrations\"]\nresolver = \"2\"\n",
                ),
                (
                    "Cargo.lock",
                    "version = 4\n\n[[package]]\nname = \"rustok-migrations\"\nversion = \"0.1.0\"\n",
                ),
                (
                    ROOT_MANIFEST,
                    "[package]\nname = \"rustok-migrations\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                ),
                (
                    "crates/utils/rustok-migrations/src/lib.rs",
                    "pub fn migration_source() {}\n",
                ),
                (".cargo/config.toml", "[alias]\nfixture = \"metadata\"\n"),
            ] {
                let file = root.join(path);
                fs::create_dir_all(file.parent().context("fixture parent missing")?)?;
                fs::write(file, bytes)?;
            }
            let cache = directory.join("cache/dep");
            fs::create_dir_all(cache.join("src"))?;
            fs::write(cache.join("Cargo.toml"), "external package")?;
            fs::write(cache.join("src/lib.rs"), "external source")?;
            let mut files = Vec::new();
            collect_snapshot_files(&root, &root, &mut files)?;
            files.sort_by(|a, b| a.path.cmp(&b.path));
            Ok(Self {
                directory,
                root,
                snapshot: snapshot(files)?,
            })
        }

        fn metadata(&self) -> Value {
            json!({
                "version": 1, "workspace_root": self.root, "workspace_members": ["root-id"],
                "packages": [
                    { "id": "root-id", "name": "rustok-migrations", "version": "0.1.0", "source": null,
                      "manifest_path": self.root.join(ROOT_MANIFEST),
                      "targets": [{ "name": "rustok_migrations", "kind": ["lib"], "crate_types": ["lib"],
                        "src_path": self.root.join("crates/utils/rustok-migrations/src/lib.rs") }] },
                    { "id": "external-id", "name": "dep", "version": "1.0.0", "source": "registry+https://example.invalid/index",
                      "manifest_path": self.directory.join("cache/dep/Cargo.toml"),
                      "targets": [{ "name": "dep", "kind": ["lib"], "crate_types": ["lib"],
                        "src_path": self.directory.join("cache/dep/src/lib.rs") }] }
                ],
                "resolve": { "nodes": [
                    { "id": "root-id", "features": ["default", "composition"],
                      "deps": [{ "name": "dep", "pkg": "external-id", "dep_kinds": [
                        { "kind": null, "target": null }, { "kind": "build", "target": "cfg(windows)" }
                      ] }] },
                    { "id": "external-id", "features": [], "deps": [] }
                ] }
            })
        }

        fn project(&self, metadata: &Value) -> Result<Value> {
            Ok(serde_json::to_value(super::project(
                &serde_json::to_vec(metadata)?,
                &self.root,
                &self.snapshot,
                "native-host",
            )?)?)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let directory = self.directory.canonicalize().expect("fixture exists");
            assert_eq!(
                directory.parent(),
                Some(std::env::temp_dir().canonicalize().unwrap().as_path())
            );
            fs::remove_dir_all(directory).expect("fixture cleanup succeeds");
        }
    }

    #[test]
    fn scratch_roots_cache_locations_and_metadata_order_do_not_change_identity() -> Result<()> {
        let first = Fixture::new()?;
        let second = Fixture::new()?;
        let mut changed_order = second.metadata();
        changed_order["packages"].as_array_mut().unwrap().reverse();
        changed_order["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .reverse();
        changed_order["resolve"]["nodes"][1]["features"]
            .as_array_mut()
            .unwrap()
            .reverse();
        changed_order["resolve"]["nodes"][1]["deps"][0]["dep_kinds"]
            .as_array_mut()
            .unwrap()
            .reverse();
        assert_eq!(
            first.project(&first.metadata())?,
            second.project(&changed_order)?
        );
        let text = serde_json::to_string(&first.project(&first.metadata())?)?;
        assert!(!text.contains(first.directory.to_str().unwrap()));
        assert!(text.contains("not an exact compilation unit graph"));
        assert!(text.contains("external package bytes"));
        Ok(())
    }

    #[test]
    fn source_lock_config_features_and_dependencies_are_bound() -> Result<()> {
        let mut fixture = Fixture::new()?;
        let metadata = fixture.metadata();
        let original = fixture.project(&metadata)?;
        for field in ["feature", "edge", "source", "target"] {
            let mut changed = metadata.clone();
            match field {
                "feature" => changed["resolve"]["nodes"][1]["features"] = json!(["extra"]),
                "edge" => {
                    changed["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["target"] =
                        json!("cfg(unix)")
                }
                "source" => {
                    changed["packages"][1]["source"] =
                        json!("git+https://example.invalid/dep#changed")
                }
                _ => changed["packages"][1]["targets"][0]["kind"] = json!(["custom-build"]),
            }
            assert_ne!(original, fixture.project(&changed)?, "{field}");
        }
        for path in [
            "Cargo.lock",
            ".cargo/config.toml",
            "crates/utils/rustok-migrations/src/lib.rs",
        ] {
            let saved = fixture.snapshot.clone();
            fixture
                .snapshot
                .files
                .iter_mut()
                .find(|file| file.path == path)
                .unwrap()
                .sha256 = digest(b"changed");
            fixture.snapshot.sha256 = snapshot(fixture.snapshot.files.clone())?.sha256;
            assert_ne!(original, fixture.project(&metadata)?, "{path}");
            fixture.snapshot = saved;
        }
        Ok(())
    }

    #[test]
    fn missing_duplicate_unknown_or_unbound_resolution_fails() -> Result<()> {
        let fixture = Fixture::new()?;
        for mutation in [
            "node",
            "package",
            "duplicate-node",
            "duplicate-package",
            "kind",
            "no-kind",
            "version",
            "root",
        ] {
            let mut metadata = fixture.metadata();
            match mutation {
                "node" => {
                    metadata["resolve"]["nodes"].as_array_mut().unwrap().pop();
                }
                "package" => {
                    metadata["packages"].as_array_mut().unwrap().pop();
                }
                "duplicate-node" => {
                    let node = metadata["resolve"]["nodes"][0].clone();
                    metadata["resolve"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .push(node);
                }
                "duplicate-package" => {
                    let package = metadata["packages"][0].clone();
                    metadata["packages"].as_array_mut().unwrap().push(package);
                }
                "kind" => {
                    metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] =
                        json!("unsupported")
                }
                "no-kind" => metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"] = json!([]),
                "version" => metadata["version"] = json!(2),
                _ => metadata["workspace_members"] = json!([]),
            }
            assert!(fixture.project(&metadata).is_err(), "{mutation}");
        }
        Ok(())
    }

    #[test]
    fn local_inputs_must_be_inside_and_present_in_snapshot() -> Result<()> {
        let mut fixture = Fixture::new()?;
        let mut outside = fixture.metadata();
        outside["packages"][1]["source"] = Value::Null;
        assert!(fixture.project(&outside).is_err());
        for missing in [
            "Cargo.lock",
            ".cargo/config.toml",
            ROOT_MANIFEST,
            "crates/utils/rustok-migrations/src/lib.rs",
        ] {
            let saved = fixture.snapshot.clone();
            fixture.snapshot.files.retain(|file| file.path != missing);
            fixture.snapshot.sha256 = snapshot(fixture.snapshot.files.clone())?.sha256;
            let result = fixture.project(&fixture.metadata());
            // Optional config absence is represented by the enclosing snapshot; required
            // manifests, the lockfile and local target sources cannot disappear.
            assert_eq!(result.is_ok(), missing == ".cargo/config.toml", "{missing}");
            fixture.snapshot = saved;
        }
        Ok(())
    }

    #[test]
    fn dev_edges_do_not_become_build_dependencies() -> Result<()> {
        let fixture = Fixture::new()?;
        let mut metadata = fixture.metadata();
        metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"] =
            json!([{ "kind": "dev", "target": null }]);
        let inputs = fixture.project(&metadata)?;
        assert_eq!(inputs["packages"].as_object().unwrap().len(), 1);
        Ok(())
    }

    #[test]
    fn unbuilt_examples_tests_and_benchmarks_need_not_exist() -> Result<()> {
        let fixture = Fixture::new()?;
        let original = fixture.project(&fixture.metadata())?;
        let mut metadata = fixture.metadata();
        for kind in ["example", "test", "bench", "bin"] {
            metadata["packages"][1]["targets"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "name": "unbuilt", "kind": [kind], "crate_types": ["bin"],
                    "src_path": fixture.directory.join("missing/../unbuilt.rs"),
                }));
        }
        assert_eq!(original, fixture.project(&metadata)?);
        metadata["packages"][1]["targets"][0]["kind"] = json!(["cdylib", "rlib"]);
        assert!(fixture.project(&metadata).is_ok());
        Ok(())
    }

    #[test]
    fn only_selected_root_binaries_are_part_of_resolution() -> Result<()> {
        let fixture = Fixture::new()?;
        let original = fixture.project(&fixture.metadata())?;
        let mut metadata = fixture.metadata();
        metadata["packages"][0]["targets"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "name": "unselected", "kind": ["bin"], "crate_types": ["bin"],
                "src_path": fixture.root.join("unbuilt.rs"),
            }));
        assert_eq!(original, fixture.project(&metadata)?);
        metadata["packages"][0]["targets"][1]["name"] = json!(BINARIES[0]);
        assert!(fixture.project(&metadata).is_err());
        metadata["packages"][0]["targets"][1]["src_path"] = json!(
            fixture
                .root
                .join("crates/utils/rustok-migrations/src/lib.rs")
        );
        assert_ne!(original, fixture.project(&metadata)?);
        Ok(())
    }

    #[test]
    fn actual_cargo_metadata_uses_the_same_projection() -> Result<()> {
        let fixture = Fixture::new()?;
        let mut rustc = build_command("rustc", &fixture.root)?;
        rustc.arg("-Vv");
        let host = native_target(&String::from_utf8(run_process(&mut rustc, 30, 4096)?)?)?;
        let inputs = collect(
            &fixture.root,
            &fixture.snapshot,
            &fixture.directory.join("target"),
            &host,
        )?;
        assert_eq!(inputs.packages.len(), 1);
        assert_eq!(inputs.source_sha256, fixture.snapshot.sha256);
        Ok(())
    }

    #[test]
    #[ignore = "reads the current repository snapshot and resolves its full Cargo metadata; no workspace compilation or database execution"]
    fn current_repository_resolution_has_captured_local_inputs() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("workspace root missing")?
            .canonicalize()?;
        let expected = working_snapshot(&root, None)?;
        let mut rustc = build_command("rustc", &root)?;
        rustc.arg("-Vv");
        let host = native_target(&String::from_utf8(run_process(&mut rustc, 30, 4096)?)?)?;
        let inputs = collect(
            &root,
            &expected,
            &root.join("target/migration-input-probe"),
            &host,
        )?;
        let local = inputs
            .packages
            .values()
            .filter(|package| matches!(package.source, PackageSource::Snapshot { .. }))
            .count();
        assert!(local > 1);
        assert!(inputs.packages.len() > local);
        assert_eq!(inputs.source_sha256, expected.sha256);
        ensure!(
            working_snapshot(&root, None)? == expected,
            "source changed during metadata probe"
        );
        println!(
            "Native normal/build closure: {} packages, {local} captured path packages; resolution {}",
            inputs.packages.len(),
            digest(&serde_json::to_vec(&inputs)?)
        );
        Ok(())
    }

    #[test]
    fn host_target_must_be_explicit_and_unambiguous() {
        assert_eq!(
            native_target("rustc version\nhost: x86_64-unknown-linux-gnu\n").unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        for value in [
            "",
            "host: \n",
            "host: --help",
            "host: --help\nhost: another",
            "host: ../escape",
        ] {
            assert!(native_target(value).is_err());
        }
    }
}
