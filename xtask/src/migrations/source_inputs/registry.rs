use super::{
    check_commit,
    inventory::{read_completed_build, Side},
    write_report,
};
use crate::migrations::{
    assert_regular, collect_snapshot_files, digest, git, read_digest, read_source, safe_relative,
};
use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(Deserialize)]
struct Resolution {
    lockfile_sha256: String,
    packages: BTreeMap<String, ResolvedPackage>,
}

#[derive(Deserialize)]
struct ResolvedPackage {
    name: String,
    version: String,
    source: PackageSource,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PackageSource {
    Snapshot {},
    External { identity: String },
}

#[derive(Deserialize)]
struct Lockfile {
    version: u32,
    package: Vec<LockedPackage>,
}

#[derive(Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(Serialize)]
struct VerifiedPackage {
    name: String,
    version: String,
    archive_sha256: String,
    source_files_sha256: String,
    source_files: usize,
}

#[derive(Serialize)]
struct Report<'a> {
    schema_version: u32,
    scope: &'static str,
    side: &'static str,
    lockfile_commit: &'a str,
    lockfile_sha256: String,
    fresh_report_sha256: String,
    build_identity_sha256: String,
    crates_io_packages: BTreeMap<String, VerifiedPackage>,
    unverified_external_packages: Vec<String>,
}

pub(in crate::migrations) fn registry_inputs(
    root: &Path,
    plan_path: &Path,
    registry_cache: &Path,
    side: &str,
) -> Result<()> {
    let side = Side::parse(side)?;
    let completed = read_completed_build(root, plan_path, side)?;
    let (commit, _) = side.source(&completed.plan);
    check_commit(root, commit)?;
    let lock = git(root, &["cat-file", "blob", &format!("{commit}:Cargo.lock")])?;
    let resolution: Resolution =
        serde_json::from_value(completed.build.evidence["source_inputs"].clone())
            .context("invalid recorded Cargo dependency resolution")?;
    ensure!(
        digest(&lock) == resolution.lockfile_sha256,
        "committed Cargo.lock does not match selected build"
    );
    let lock = lock_checksums(&lock)?;
    let cache = root
        .join(registry_cache)
        .canonicalize()
        .context("registry cache directory unavailable")?;
    let extraction = matching_extraction_root(&cache)?;
    let mut verified = BTreeMap::new();
    let mut unverified = Vec::new();
    for (id, package) in resolution.packages {
        let PackageSource::External { identity } = package.source else {
            continue;
        };
        if identity != CRATES_IO {
            unverified.push(id);
            continue;
        }
        let key = (package.name, package.version);
        let checksum = lock.get(&key).with_context(|| {
            format!(
                "crates.io package {} {} is missing from Cargo.lock",
                key.0, key.1
            )
        })?;
        let evidence = verify_package(&cache, &extraction, &key.0, &key.1, checksum)
            .with_context(|| format!("crates.io package {} {}", key.0, key.1))?;
        verified.insert(id, evidence);
    }
    write_report(&Report {
        schema_version: 1,
        scope: "current-checked-cargo-registry-cache",
        side: side.name(),
        lockfile_commit: commit,
        lockfile_sha256: resolution.lockfile_sha256,
        fresh_report_sha256: completed.fresh_report_sha256,
        build_identity_sha256: completed.build.sha256,
        crates_io_packages: verified,
        unverified_external_packages: unverified,
    })
}

fn lock_checksums(bytes: &[u8]) -> Result<BTreeMap<(String, String), String>> {
    let lock: Lockfile =
        toml::from_str(std::str::from_utf8(bytes)?).context("invalid Cargo.lock")?;
    ensure!(
        matches!(lock.version, 3 | 4),
        "unsupported Cargo.lock version"
    );
    let mut checksums = BTreeMap::new();
    for entry in lock.package {
        if entry.source.as_deref() != Some(CRATES_IO) {
            continue;
        }
        let checksum = entry
            .checksum
            .context("registry package has no Cargo.lock checksum")?;
        ensure!(
            checksum.len() == 64
                && checksum
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
            "invalid Cargo.lock package SHA-256"
        );
        ensure!(
            checksums
                .insert((entry.name, entry.version), checksum)
                .is_none(),
            "duplicate crates.io package in Cargo.lock"
        );
    }
    Ok(checksums)
}

fn matching_extraction_root(cache: &Path) -> Result<PathBuf> {
    let bucket = cache.file_name().context("cache bucket name missing")?;
    let cache_parent = cache.parent().context("cache directory has no parent")?;
    let registry = cache_parent
        .parent()
        .context("registry directory missing")?;
    ensure!(
        cache_parent.ends_with("cache") && registry.ends_with("registry") && cache.is_dir(),
        "--registry-cache must select a Cargo registry/cache/<index> directory"
    );
    let extraction = registry.join("src").join(bucket);
    ensure!(
        extraction.canonicalize()? == extraction && extraction.is_dir(),
        "paired registry/src directory moved or redirected"
    );
    Ok(extraction)
}

fn verify_package(
    cache: &Path,
    extracted: &Path,
    name: &str,
    version: &str,
    checksum: &str,
) -> Result<VerifiedPackage> {
    ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "invalid registry package name"
    );
    semver::Version::parse(version).context("invalid registry package version")?;
    let directory = format!("{name}-{version}");
    let archive = safe_relative(&format!("{directory}.crate"))?;
    assert_regular(cache, &archive)?;
    let bytes = read_source(&cache.join(archive))?;
    ensure!(
        digest(&bytes) == checksum,
        "cached crate archive differs from Cargo.lock"
    );
    let expected = archive_files(&directory, &bytes)?;

    let source = extracted.join(directory);
    ensure!(
        source.canonicalize()? == source && source.is_dir(),
        "extracted package directory moved or redirected"
    );
    let mut actual = Vec::new();
    collect_snapshot_files(&source, &source, &mut actual)?;
    let actual: BTreeMap<_, _> = actual
        .into_iter()
        .filter(|file| file.path != ".cargo-ok" || expected.contains_key(".cargo-ok"))
        .map(|file| (file.path, file.sha256))
        .collect();
    for (path, expected) in &expected {
        let found = actual
            .get(path)
            .with_context(|| format!("extracted source file missing: {path}"))?;
        ensure!(
            found == expected,
            "extracted source bytes differ from archive: {path}"
        );
    }
    for path in actual.keys() {
        ensure!(
            expected.contains_key(path),
            "file absent from archive exists in extracted source: {path}"
        );
    }
    Ok(VerifiedPackage {
        name: name.to_owned(),
        version: version.to_owned(),
        archive_sha256: checksum.to_owned(),
        source_files_sha256: digest(&serde_json::to_vec(&expected)?),
        source_files: expected.len(),
    })
}

fn archive_files(directory: &str, bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let prefix = format!("{directory}/");
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let mut contents = BTreeMap::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let entry_type = entry.header().entry_type();
        let path = String::from_utf8(entry.path_bytes().into_owned())?;
        if entry_type.is_dir() && path.trim_end_matches('/') == directory {
            continue;
        }
        let relative = path
            .strip_prefix(&prefix)
            .context("archive entry is outside its package directory")?;
        if entry_type.is_dir() {
            safe_relative(relative.trim_end_matches('/'))?;
            continue;
        }
        safe_relative(relative)?;
        ensure!(
            entry_type.is_file(),
            "unsupported crate archive entry: {relative}"
        );
        ensure!(
            contents
                .insert(relative.to_owned(), read_digest(entry)?)
                .is_none(),
            "duplicate crate archive entry: {relative}"
        );
    }
    ensure!(
        contents.contains_key("Cargo.toml"),
        "crate archive has no Cargo.toml"
    );
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::{
        env, fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn archive(paths: &[(&str, &[u8])]) -> Result<Vec<u8>> {
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (path, contents) in paths {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            archive.append_data(&mut header, path, *contents)?;
        }
        Ok(archive.into_inner()?.finish()?)
    }

    #[test]
    fn verifies_extracted_cache_against_the_locked_archive() -> Result<()> {
        let temp = env::temp_dir().canonicalize()?;
        let fixture = temp.join(format!(
            "rustok-registry-test-{}-{}",
            process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&fixture)?;
        let outcome = (|| {
            let cache = fixture.join("registry/cache/index");
            let extraction = fixture.join("registry/src/index");
            let package = extraction.join("crate-demo-1.0.0");
            fs::create_dir_all(&cache)?;
            fs::create_dir_all(package.join("src"))?;
            let bytes = archive(&[
                ("crate-demo-1.0.0/Cargo.toml", b"manifest"),
                ("crate-demo-1.0.0/src/lib.rs", b"original"),
            ])?;
            let checksum = digest(&bytes);
            fs::write(cache.join("crate-demo-1.0.0.crate"), &bytes)?;
            fs::write(package.join("Cargo.toml"), b"manifest")?;
            fs::write(package.join("src/lib.rs"), b"original")?;
            fs::write(package.join(".cargo-ok"), br#"{"v":1}"#)?;
            assert_eq!(matching_extraction_root(&cache)?, extraction);
            let verified = verify_package(&cache, &extraction, "crate-demo", "1.0.0", &checksum)?;
            assert_eq!(verified.source_files, 2);
            assert!(verify_package(
                &cache,
                &extraction,
                "crate-demo",
                "1.0.0",
                &digest(b"wrong archive")
            )
            .is_err());

            fs::write(package.join("src/lib.rs"), b"modified")?;
            let error = verify_package(&cache, &extraction, "crate-demo", "1.0.0", &checksum)
                .err()
                .context("modified extraction accepted")?;
            assert!(error
                .to_string()
                .contains("bytes differ from archive: src/lib.rs"));
            fs::write(package.join("src/lib.rs"), b"original")?;
            fs::write(package.join("src/added.rs"), b"added")?;
            let error = verify_package(&cache, &extraction, "crate-demo", "1.0.0", &checksum)
                .err()
                .context("extra cache file accepted")?;
            assert!(error.to_string().contains("file absent from archive"));
            Ok(())
        })();
        ensure!(
            fixture.canonicalize()?.parent() == Some(temp.as_path()),
            "fixture escaped its temporary directory"
        );
        fs::remove_dir_all(fixture)?;
        outcome
    }

    #[test]
    fn rejects_archive_entries_outside_package_and_duplicate_files() -> Result<()> {
        let outside = archive(&[("other-1.0.0/Cargo.toml", b"manifest")])?;
        assert!(archive_files("crate-demo-1.0.0", &outside).is_err());
        let repeat = archive(&[
            ("crate-demo-1.0.0/Cargo.toml", b"manifest"),
            ("crate-demo-1.0.0/Cargo.toml", b"duplicate"),
        ])?;
        assert!(archive_files("crate-demo-1.0.0", &repeat).is_err());
        Ok(())
    }

    #[test]
    fn reads_only_the_checksums_of_the_exact_registry() -> Result<()> {
        let hash = digest(b"archive");
        let lock = format!(
            r#"
            version = 4
            [[package]]
            name = "example"
            version = "1.0.0"
            source = "{CRATES_IO}"
            checksum = "{hash}"
            [[package]]
            name = "example"
            version = "1.0.0"
            source = "git+https://example.invalid"
        "#
        );
        assert_eq!(
            lock_checksums(lock.as_bytes())?,
            BTreeMap::from([(("example".into(), "1.0.0".into()), hash)])
        );
        Ok(())
    }
}
